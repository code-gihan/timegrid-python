#![allow(non_snake_case, non_upper_case_globals)]

use chrono::{Datelike, Days, Duration, NaiveDate, NaiveDateTime, NaiveTime};
use pyo3::Bound;
use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyOverflowError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDelta, PyTuple};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const DEFAULT_CAPACITY: i32 = 1;
const DEFAULT_SEARCH_DAYS: i64 = 7;
const DEFAULT_SEARCH_YEARS_DAYS: i64 = 366 * 5;
const TICKS_PER_MICROSECOND: i64 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Window {
    start: NaiveDateTime,
    end: NaiveDateTime,
}

impl Window {
    fn new(start: NaiveDateTime, end: NaiveDateTime) -> PyResult<Self> {
        if end < start {
            return Err(PyValueError::new_err(
                "End must be greater than or equal to start.",
            ));
        }
        Ok(Self { start, end })
    }

    fn duration(self) -> Duration {
        self.end - self.start
    }

    fn is_empty(self) -> bool {
        self.end <= self.start
    }

    fn contains(self, instant: NaiveDateTime) -> bool {
        self.start <= instant && instant < self.end
    }

    fn overlaps(self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }

    fn intersect(self, other: Self) -> Option<Self> {
        let start = self.start.max(other.start);
        let end = self.end.min(other.end);
        (start < end).then_some(Self { start, end })
    }
}

#[pyclass(module = "timegrid")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeWindow {
    inner: Window,
}

#[pymethods]
impl TimeWindow {
    #[new]
    fn new(start: NaiveDateTime, end: NaiveDateTime) -> PyResult<Self> {
        Ok(Self {
            inner: Window::new(start, end)?,
        })
    }

    #[getter]
    fn Start(&self) -> NaiveDateTime {
        self.inner.start
    }

    #[getter]
    fn End(&self) -> NaiveDateTime {
        self.inner.end
    }

    #[getter]
    fn Duration<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDelta>> {
        duration_to_py(py, self.inner.duration())
    }

    #[getter]
    fn IsEmpty(&self) -> bool {
        self.inner.is_empty()
    }

    fn Contains(&self, instant: NaiveDateTime) -> bool {
        self.inner.contains(instant)
    }

    fn Overlaps(&self, other: &TimeWindow) -> bool {
        self.inner.overlaps(other.inner)
    }

    fn Intersect(&self, other: &TimeWindow) -> Option<TimeWindow> {
        self.inner.intersect(other.inner).map(TimeWindow::from)
    }

    fn __richcmp__(&self, other: PyRef<TimeWindow>, op: CompareOp) -> PyResult<bool> {
        match op {
            CompareOp::Eq => Ok(self.inner == other.inner),
            CompareOp::Ne => Ok(self.inner != other.inner),
            _ => Err(PyValueError::new_err(
                "TimeWindow only supports equality comparison.",
            )),
        }
    }

    fn __repr__(&self) -> String {
        format!("TimeWindow({}, {})", self.inner.start, self.inner.end)
    }
}

impl From<Window> for TimeWindow {
    fn from(inner: Window) -> Self {
        Self { inner }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct WeeklyRule {
    day: u8,
    start: NaiveTime,
    end: NaiveTime,
}

impl WeeklyRule {
    fn new(day: u8, start: NaiveTime, end: NaiveTime) -> PyResult<Self> {
        if day > 6 {
            return Err(PyValueError::new_err(
                "Day must be in DayOfWeek range 0..6.",
            ));
        }
        if start == end {
            return Err(PyValueError::new_err(
                "Start and end cannot be the same time.",
            ));
        }
        Ok(Self { day, start, end })
    }

    fn contains(self, instant: NaiveDateTime) -> bool {
        let time = instant.time();
        let day = day_of_week(instant.date());
        if self.start < self.end {
            day == self.day && self.start <= time && time < self.end
        } else {
            (day == self.day && self.start <= time)
                || (day == next_day(self.day) && time < self.end)
        }
    }

    fn to_window(self, date: NaiveDate) -> Window {
        let start = date.and_time(self.start);
        let end = if self.start < self.end {
            date.and_time(self.end)
        } else {
            date.checked_add_days(Days::new(1))
                .unwrap_or(date)
                .and_time(self.end)
        };
        Window { start, end }
    }
}

#[pyclass(module = "timegrid")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeGridWeeklyRule {
    inner: WeeklyRule,
}

#[pymethods]
impl TimeGridWeeklyRule {
    #[new]
    fn new(day: u8, start: NaiveTime, end: NaiveTime) -> PyResult<Self> {
        Ok(Self {
            inner: WeeklyRule::new(day, start, end)?,
        })
    }

    #[getter]
    fn Day(&self) -> u8 {
        self.inner.day
    }

    #[getter]
    fn Start(&self) -> NaiveTime {
        self.inner.start
    }

    #[getter]
    fn End(&self) -> NaiveTime {
        self.inner.end
    }

    fn Contains(&self, instant: NaiveDateTime) -> bool {
        self.inner.contains(instant)
    }

    fn ToWindow(&self, date: NaiveDate) -> TimeWindow {
        self.inner.to_window(date).into()
    }
}

impl From<WeeklyRule> for TimeGridWeeklyRule {
    fn from(inner: WeeklyRule) -> Self {
        Self { inner }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct CapacityWindow {
    window: Window,
    capacity: i32,
}

#[pyclass(module = "timegrid")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeGridCapacityWindow {
    inner: CapacityWindow,
}

#[pymethods]
impl TimeGridCapacityWindow {
    #[getter]
    fn Window(&self) -> TimeWindow {
        self.inner.window.into()
    }

    #[getter]
    fn Capacity(&self) -> i32 {
        self.inner.capacity
    }
}

impl From<CapacityWindow> for TimeGridCapacityWindow {
    fn from(inner: CapacityWindow) -> Self {
        Self { inner }
    }
}

#[pyclass(module = "timegrid")]
pub struct DayOfWeek;

#[pymethods]
impl DayOfWeek {
    #[classattr]
    const Sunday: u8 = 0;
    #[classattr]
    const Monday: u8 = 1;
    #[classattr]
    const Tuesday: u8 = 2;
    #[classattr]
    const Wednesday: u8 = 3;
    #[classattr]
    const Thursday: u8 = 4;
    #[classattr]
    const Friday: u8 = 5;
    #[classattr]
    const Saturday: u8 = 6;
}

#[pyclass(module = "timegrid")]
pub struct TimeGridEntryKind;

#[pymethods]
impl TimeGridEntryKind {
    #[classattr]
    const Open: u8 = 0;
    #[classattr]
    const Closed: u8 = 1;
    #[classattr]
    const Capacity: u8 = 2;
}

#[pyclass(module = "timegrid")]
pub struct TimeGridWindowKind;

#[pymethods]
impl TimeGridWindowKind {
    #[classattr]
    const Available: u8 = 0;
    #[classattr]
    const Unavailable: u8 = 1;
    #[classattr]
    const OpenRule: u8 = 2;
    #[classattr]
    const OpenWindow: u8 = 3;
    #[classattr]
    const BreakRule: u8 = 4;
    #[classattr]
    const ClosedWindow: u8 = 5;
    #[classattr]
    const CapacityOverride: u8 = 6;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    id: String,
    kind: u8,
    window: Window,
    capacity: i32,
}

#[pyclass(module = "timegrid")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeGridEntry {
    inner: Entry,
}

#[pymethods]
impl TimeGridEntry {
    #[getter]
    fn Id(&self) -> String {
        self.inner.id.clone()
    }

    #[getter]
    fn Kind(&self) -> u8 {
        self.inner.kind
    }

    #[getter]
    fn Window(&self) -> TimeWindow {
        self.inner.window.into()
    }

    #[getter]
    fn Capacity(&self) -> i32 {
        self.inner.capacity
    }
}

impl From<Entry> for TimeGridEntry {
    fn from(inner: Entry) -> Self {
        Self { inner }
    }
}

#[derive(Clone, Debug)]
struct Calendar {
    default_capacity: i32,
    open_rules: Vec<WeeklyRule>,
    break_rules: Vec<WeeklyRule>,
    open_windows: Vec<Window>,
    closed_windows: Vec<Window>,
    capacity_overrides: Vec<CapacityWindow>,
    entries: Vec<Entry>,
    components: Vec<Calendar>,
}

impl Default for Calendar {
    fn default() -> Self {
        Self {
            default_capacity: DEFAULT_CAPACITY,
            open_rules: Vec::new(),
            break_rules: Vec::new(),
            open_windows: Vec::new(),
            closed_windows: Vec::new(),
            capacity_overrides: Vec::new(),
            entries: Vec::new(),
            components: Vec::new(),
        }
    }
}

impl Calendar {
    fn ensure_editable(&self) -> PyResult<()> {
        if self.is_composite() {
            Err(PyRuntimeError::new_err(
                "Composite calendars are query-only. Add rules to source calendars before composing.",
            ))
        } else {
            Ok(())
        }
    }

    fn is_composite(&self) -> bool {
        !self.components.is_empty()
    }

    fn set_default_capacity(&mut self, capacity: i32) -> PyResult<()> {
        self.ensure_editable()?;
        if capacity < 0 {
            return Err(PyValueError::new_err("Capacity cannot be negative."));
        }
        self.default_capacity = capacity;
        Ok(())
    }

    fn add_open_rule(&mut self, day: u8, start: NaiveTime, end: NaiveTime) -> PyResult<()> {
        self.ensure_editable()?;
        self.open_rules.push(WeeklyRule::new(day, start, end)?);
        Ok(())
    }

    fn add_break_rule(&mut self, day: u8, start: NaiveTime, end: NaiveTime) -> PyResult<()> {
        self.ensure_editable()?;
        self.break_rules.push(WeeklyRule::new(day, start, end)?);
        Ok(())
    }

    fn add_weekday_open_rule(&mut self, start: NaiveTime, end: NaiveTime) -> PyResult<()> {
        for day in 1..=5 {
            self.add_open_rule(day, start, end)?;
        }
        Ok(())
    }

    fn add_weekday_break_rule(&mut self, start: NaiveTime, end: NaiveTime) -> PyResult<()> {
        for day in 1..=5 {
            self.add_break_rule(day, start, end)?;
        }
        Ok(())
    }

    fn add_open_window(&mut self, start: NaiveDateTime, end: NaiveDateTime) -> PyResult<()> {
        self.ensure_editable()?;
        self.open_windows.push(Window::new(start, end)?);
        Ok(())
    }

    fn add_closed_window(&mut self, start: NaiveDateTime, end: NaiveDateTime) -> PyResult<()> {
        self.ensure_editable()?;
        self.closed_windows.push(Window::new(start, end)?);
        Ok(())
    }

    fn add_holiday(&mut self, date: NaiveDate) -> PyResult<()> {
        let start = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| PyOverflowError::new_err("Invalid date."))?;
        let end = date
            .checked_add_days(Days::new(1))
            .ok_or_else(|| PyOverflowError::new_err("Date overflow."))?
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| PyOverflowError::new_err("Invalid date."))?;
        self.add_closed_window(start, end)
    }

    fn add_capacity_override(
        &mut self,
        start: NaiveDateTime,
        end: NaiveDateTime,
        capacity: i32,
    ) -> PyResult<()> {
        self.ensure_editable()?;
        if capacity < 0 {
            return Err(PyValueError::new_err("Capacity cannot be negative."));
        }
        self.capacity_overrides.push(CapacityWindow {
            window: Window::new(start, end)?,
            capacity,
        });
        Ok(())
    }

    fn set_entry(
        &mut self,
        id: String,
        kind: u8,
        start: NaiveDateTime,
        end: NaiveDateTime,
        capacity: i32,
    ) -> PyResult<()> {
        self.ensure_editable()?;
        validate_entry_id(&id)?;
        if kind == 2 && capacity < 0 {
            return Err(PyValueError::new_err("Capacity cannot be negative."));
        }
        let entry = Entry {
            id: id.clone(),
            kind,
            window: Window::new(start, end)?,
            capacity,
        };
        if let Some(index) = self.entries.iter().position(|e| e.id == id) {
            self.entries[index] = entry;
        } else {
            self.entries.push(entry);
        }
        Ok(())
    }

    fn get_capacity_at(&self, instant: NaiveDateTime) -> i32 {
        if self.is_composite() {
            let mut capacity = i32::MAX;
            for component in &self.components {
                let component_capacity = component.get_capacity_at(instant);
                if component_capacity == 0 {
                    return 0;
                }
                capacity = capacity.min(component_capacity);
            }
            return if capacity == i32::MAX { 0 } else { capacity };
        }

        if !self.is_open_by_calendar(instant) {
            return 0;
        }

        let mut capacity = self.default_capacity;
        for override_window in &self.capacity_overrides {
            if override_window.window.contains(instant) {
                capacity = override_window.capacity;
            }
        }
        for entry in &self.entries {
            if entry.kind == 2 && entry.window.contains(instant) {
                capacity = entry.capacity;
            }
        }
        capacity
    }

    fn is_open_by_calendar(&self, instant: NaiveDateTime) -> bool {
        if self.is_composite() {
            return self
                .components
                .iter()
                .all(|c| c.get_capacity_at(instant) > 0);
        }

        let mut has_open = self.open_rules.iter().any(|r| r.contains(instant))
            || self.open_windows.iter().any(|w| w.contains(instant))
            || self
                .entries
                .iter()
                .any(|e| e.kind == 0 && e.window.contains(instant));

        if !has_open {
            return false;
        }

        has_open = !self.break_rules.iter().any(|r| r.contains(instant))
            && !self.closed_windows.iter().any(|w| w.contains(instant))
            && !self
                .entries
                .iter()
                .any(|e| e.kind == 1 && e.window.contains(instant));
        has_open
    }

    fn get_next_open_time(
        &self,
        start: NaiveDateTime,
        search_until: Option<NaiveDateTime>,
    ) -> PyResult<Option<NaiveDateTime>> {
        let until =
            search_until.unwrap_or_else(|| start + Duration::days(DEFAULT_SEARCH_YEARS_DAYS));
        for chunk in search_chunks(start, until, Duration::zero()) {
            let windows = self.get_open_windows(chunk.start, chunk.end)?;
            if let Some(first) = windows.first() {
                return Ok(Some(if first.contains(start) {
                    start
                } else {
                    first.start
                }));
            }
        }
        Ok(None)
    }

    fn add_work_duration(
        &self,
        start: NaiveDateTime,
        duration: Duration,
        search_until: Option<NaiveDateTime>,
    ) -> PyResult<NaiveDateTime> {
        if duration < Duration::zero() {
            return Err(PyValueError::new_err("Duration cannot be negative."));
        }
        if duration == Duration::zero() {
            return Ok(start);
        }

        let until =
            search_until.unwrap_or_else(|| start + Duration::days(DEFAULT_SEARCH_YEARS_DAYS));
        let mut remaining = duration;
        for chunk in search_chunks(start, until, Duration::zero()) {
            for window in self.get_open_windows(chunk.start, chunk.end)? {
                if remaining <= window.duration() {
                    return Ok(window.start + remaining);
                }
                remaining = remaining - window.duration();
            }
        }
        Err(PyRuntimeError::new_err(
            "The requested working duration was not found inside the search range.",
        ))
    }

    fn trace_work_duration(
        &self,
        start: NaiveDateTime,
        duration: Duration,
        search_until: Option<NaiveDateTime>,
    ) -> PyResult<WorkingTimeTrace> {
        if duration < Duration::zero() {
            return Err(PyValueError::new_err("Duration cannot be negative."));
        }
        if duration == Duration::zero() {
            return Ok(WorkingTimeTrace {
                start,
                requested_duration: duration,
                result: start,
                steps: Vec::new(),
            });
        }

        let until =
            search_until.unwrap_or_else(|| start + Duration::days(DEFAULT_SEARCH_YEARS_DAYS));
        let mut remaining = duration;
        let mut steps = Vec::new();
        for chunk in search_chunks(start, until, Duration::zero()) {
            for window in self.get_open_windows(chunk.start, chunk.end)? {
                let consumed = if remaining < window.duration() {
                    remaining
                } else {
                    window.duration()
                };
                if consumed > Duration::zero() {
                    steps.push(WorkingTimeTraceStep {
                        window: Window {
                            start: window.start,
                            end: window.start + consumed,
                        },
                        duration: consumed,
                    });
                }
                if remaining <= window.duration() {
                    return Ok(WorkingTimeTrace {
                        start,
                        requested_duration: duration,
                        result: window.start + remaining,
                        steps,
                    });
                }
                remaining = remaining - window.duration();
            }
        }
        Err(PyRuntimeError::new_err(
            "The requested working duration was not found inside the search range.",
        ))
    }

    fn get_working_duration(&self, start: NaiveDateTime, end: NaiveDateTime) -> PyResult<Duration> {
        let mut total = Duration::zero();
        for window in self.get_open_windows(start, end)? {
            total = total + window.duration();
        }
        Ok(total)
    }

    fn get_open_windows(&self, start: NaiveDateTime, end: NaiveDateTime) -> PyResult<Vec<Window>> {
        self.windows_with_capacity(start, end, 1)
    }

    fn get_capacity_windows(
        &self,
        start: NaiveDateTime,
        end: NaiveDateTime,
        minimum_capacity: i32,
    ) -> PyResult<Vec<Window>> {
        validate_positive_capacity(minimum_capacity)?;
        self.windows_with_capacity(start, end, minimum_capacity)
    }

    fn get_unavailable_windows(
        &self,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<Vec<Window>> {
        let query = Window::new(start, end)?;
        if query.is_empty() {
            return Ok(Vec::new());
        }
        Ok(complement(query, &self.get_open_windows(start, end)?))
    }

    fn get_state_windows(
        &self,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<Vec<StateWindow>> {
        self.state_windows(start, end)
    }

    fn get_windows_at(
        &self,
        instant: NaiveDateTime,
        search_radius: Option<Duration>,
    ) -> PyResult<Vec<WindowMatch>> {
        let mut result = Vec::new();
        let capacity = self.get_capacity_at(instant);
        let effective = if capacity > 0 {
            self.get_current_open_window(instant, search_radius)?
        } else {
            self.get_current_unavailable_window(instant, search_radius)?
        };
        if let Some(window) = effective {
            result.push(WindowMatch {
                kind: if capacity > 0 { 0 } else { 1 },
                window,
                capacity: Some(capacity),
            });
        }
        self.add_raw_window_matches(instant, &mut result);
        Ok(result)
    }

    fn get_current_open_window(
        &self,
        instant: NaiveDateTime,
        search_radius: Option<Duration>,
    ) -> PyResult<Option<Window>> {
        self.get_current_state_window(instant, search_radius, true)
    }

    fn get_current_unavailable_window(
        &self,
        instant: NaiveDateTime,
        search_radius: Option<Duration>,
    ) -> PyResult<Option<Window>> {
        self.get_current_state_window(instant, search_radius, false)
    }

    fn get_next_transition_time(
        &self,
        instant: NaiveDateTime,
        search_radius: Option<Duration>,
    ) -> PyResult<Option<NaiveDateTime>> {
        let query = search_window_around(instant, search_radius)?;
        let mut transition = None;
        for state in self.state_windows(query.start, query.end)? {
            if state.window.start != query.start && state.window.start > instant {
                transition = Some(min_dt(transition, state.window.start));
            }
            if state.window.end != query.end && state.window.end > instant {
                transition = Some(min_dt(transition, state.window.end));
            }
        }
        Ok(transition)
    }

    fn get_previous_transition_time(
        &self,
        instant: NaiveDateTime,
        search_radius: Option<Duration>,
    ) -> PyResult<Option<NaiveDateTime>> {
        let query = search_window_around(instant, search_radius)?;
        let mut transition = None;
        for state in self.state_windows(query.start, query.end)? {
            if state.window.start != query.start && state.window.start < instant {
                transition = Some(max_dt(transition, state.window.start));
            }
            if state.window.end != query.end && state.window.end < instant {
                transition = Some(max_dt(transition, state.window.end));
            }
        }
        Ok(transition)
    }

    fn get_nearest_transition_time(
        &self,
        instant: NaiveDateTime,
        search_radius: Option<Duration>,
    ) -> PyResult<Option<NaiveDateTime>> {
        let query = search_window_around(instant, search_radius)?;
        for state in self.state_windows(query.start, query.end)? {
            if (state.window.start == instant && state.window.start != query.start)
                || (state.window.end == instant && state.window.end != query.end)
            {
                return Ok(Some(instant));
            }
        }
        let previous = self.get_previous_transition_time(instant, search_radius)?;
        let next = self.get_next_transition_time(instant, search_radius)?;
        Ok(match (previous, next) {
            (None, n) => n,
            (p, None) => p,
            (Some(p), Some(n)) => {
                if instant - p <= n - instant {
                    Some(p)
                } else {
                    Some(n)
                }
            }
        })
    }

    fn find_first_slot(
        &self,
        start: NaiveDateTime,
        duration: Duration,
        minimum_capacity: i32,
        search_until: Option<NaiveDateTime>,
    ) -> PyResult<Option<Window>> {
        if duration < Duration::zero() {
            return Err(PyValueError::new_err("Duration cannot be negative."));
        }
        validate_positive_capacity(minimum_capacity)?;
        let until =
            search_until.unwrap_or_else(|| start + Duration::days(DEFAULT_SEARCH_YEARS_DAYS));
        for chunk in search_chunks(start, until, duration) {
            for window in self.windows_with_capacity(chunk.start, chunk.end, minimum_capacity)? {
                if duration == Duration::zero() {
                    return Ok(Some(Window {
                        start: window.start,
                        end: window.start,
                    }));
                }
                if window.duration() >= duration {
                    return Ok(Some(Window {
                        start: window.start,
                        end: window.start + duration,
                    }));
                }
            }
        }
        Ok(None)
    }

    fn state_windows(
        &self,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<Vec<StateWindow>> {
        let query = Window::new(start, end)?;
        if query.is_empty() {
            return Ok(Vec::new());
        }
        if self.is_composite() {
            return self.composite_state_windows(query);
        }

        let open_windows = self.calendar_windows(query)?;
        let capacity_sources = self.capacity_sources(query);
        let mut points = vec![query.start, query.end];
        for window in &open_windows {
            points.push(window.start);
            points.push(window.end);
        }

        let mut events = Vec::with_capacity(capacity_sources.len() * 2);
        for source in &capacity_sources {
            points.push(source.window.start);
            points.push(source.window.end);
            events.push(CapacityEvent {
                time: source.window.start,
                order: source.order,
                is_start: true,
            });
            events.push(CapacityEvent {
                time: source.window.end,
                order: source.order,
                is_start: false,
            });
        }
        points.sort();
        points.dedup();
        events.sort_by(|a, b| {
            a.time
                .cmp(&b.time)
                .then_with(|| a.is_start.cmp(&b.is_start))
                .then_with(|| a.order.cmp(&b.order))
        });

        let mut result = Vec::new();
        let mut previous = points[0];
        let mut open_index = 0usize;
        let mut event_index = 0usize;
        let mut active = BTreeSet::<usize>::new();

        for current in points.into_iter().skip(1) {
            while event_index < events.len() && events[event_index].time == previous {
                if events[event_index].is_start {
                    active.insert(events[event_index].order);
                } else {
                    active.remove(&events[event_index].order);
                }
                event_index += 1;
            }
            while open_index < open_windows.len() && open_windows[open_index].end <= previous {
                open_index += 1;
            }
            let mut capacity = 0;
            if open_index < open_windows.len()
                && open_windows[open_index].start <= previous
                && current <= open_windows[open_index].end
            {
                capacity = active
                    .iter()
                    .next_back()
                    .map(|order| capacity_sources[*order].capacity)
                    .unwrap_or(self.default_capacity);
            }
            add_state_window(
                &mut result,
                StateWindow {
                    window: Window {
                        start: previous,
                        end: current,
                    },
                    capacity,
                },
            );
            previous = current;
        }

        Ok(result)
    }

    fn composite_state_windows(&self, query: Window) -> PyResult<Vec<StateWindow>> {
        let mut points = vec![query.start, query.end];
        for component in &self.components {
            for state in component.state_windows(query.start, query.end)? {
                points.push(state.window.start);
                points.push(state.window.end);
            }
        }
        points.sort();
        points.dedup();
        let mut result = Vec::new();
        let mut previous = points[0];
        for current in points.into_iter().skip(1) {
            add_state_window(
                &mut result,
                StateWindow {
                    window: Window {
                        start: previous,
                        end: current,
                    },
                    capacity: self.get_capacity_at(previous),
                },
            );
            previous = current;
        }
        Ok(result)
    }

    fn windows_with_capacity(
        &self,
        start: NaiveDateTime,
        end: NaiveDateTime,
        minimum_capacity: i32,
    ) -> PyResult<Vec<Window>> {
        Ok(merge(
            self.state_windows(start, end)?
                .into_iter()
                .filter(|state| state.capacity >= minimum_capacity)
                .map(|state| state.window)
                .collect(),
        ))
    }

    fn get_current_state_window(
        &self,
        instant: NaiveDateTime,
        search_radius: Option<Duration>,
        can_work: bool,
    ) -> PyResult<Option<Window>> {
        let query = search_window_around(instant, search_radius)?;
        Ok(self
            .state_windows(query.start, query.end)?
            .into_iter()
            .find(|state| state.can_work() == can_work && state.window.contains(instant))
            .map(|state| state.window))
    }

    fn calendar_windows(&self, query: Window) -> PyResult<Vec<Window>> {
        if self.is_composite() {
            return self.composite_calendar_windows(query);
        }

        let mut open_windows = expand_rules(&self.open_rules, query);
        add_clipped_windows(&self.open_windows, query, &mut open_windows);
        self.add_entry_windows(0, query, &mut open_windows);

        let mut closed_windows = expand_rules(&self.break_rules, query);
        closed_windows.extend(self.closed_windows.iter().copied());
        self.add_entry_windows(1, query, &mut closed_windows);
        let mut clipped_closed: Vec<_> = closed_windows
            .into_iter()
            .filter_map(|window| window.intersect(query))
            .collect();
        clipped_closed.sort_by(compare_windows);

        let mut available = Vec::new();
        for open_window in open_windows {
            let mut parts = vec![open_window];
            for closed in &clipped_closed {
                let mut next_parts = Vec::new();
                for part in parts {
                    subtract_into(part, *closed, &mut next_parts);
                }
                parts = next_parts;
                if parts.is_empty() {
                    break;
                }
            }
            available.extend(parts);
        }
        Ok(merge(available))
    }

    fn composite_calendar_windows(&self, query: Window) -> PyResult<Vec<Window>> {
        let mut windows = self.components[0].calendar_windows(query)?;
        for component in self.components.iter().skip(1) {
            if windows.is_empty() {
                break;
            }
            windows = intersect_windows(&windows, &component.calendar_windows(query)?);
        }
        Ok(windows)
    }

    fn capacity_sources(&self, query: Window) -> Vec<CapacitySource> {
        let mut result = Vec::new();
        for source in &self.capacity_overrides {
            if let Some(window) = source.window.intersect(query) {
                result.push(CapacitySource {
                    window,
                    capacity: source.capacity,
                    order: result.len(),
                });
            }
        }
        for entry in &self.entries {
            if entry.kind == 2 {
                if let Some(window) = entry.window.intersect(query) {
                    result.push(CapacitySource {
                        window,
                        capacity: entry.capacity,
                        order: result.len(),
                    });
                }
            }
        }
        result
    }

    fn add_entry_windows(&self, kind: u8, query: Window, result: &mut Vec<Window>) {
        for entry in &self.entries {
            if entry.kind == kind {
                if let Some(window) = entry.window.intersect(query) {
                    result.push(window);
                }
            }
        }
    }

    fn add_raw_window_matches(&self, instant: NaiveDateTime, result: &mut Vec<WindowMatch>) {
        if self.is_composite() {
            for component in &self.components {
                component.add_raw_window_matches(instant, result);
            }
            return;
        }

        add_rule_matches(&self.open_rules, 2, instant, result);
        add_rule_matches(&self.break_rules, 4, instant, result);
        for window in &self.closed_windows {
            if window.contains(instant) {
                result.push(WindowMatch {
                    kind: 5,
                    window: *window,
                    capacity: Some(0),
                });
            }
        }
        for entry in &self.entries {
            if entry.window.contains(instant) {
                result.push(WindowMatch {
                    kind: match entry.kind {
                        0 => 3,
                        1 => 5,
                        2 => 6,
                        _ => 5,
                    },
                    window: entry.window,
                    capacity: (entry.kind == 2).then_some(entry.capacity),
                });
            }
        }
        for override_window in &self.capacity_overrides {
            if override_window.window.contains(instant) {
                result.push(WindowMatch {
                    kind: 6,
                    window: override_window.window,
                    capacity: Some(override_window.capacity),
                });
            }
        }
    }
}

#[pyclass(module = "timegrid")]
#[derive(Clone, Debug)]
pub struct TimeGridCalendar {
    inner: Calendar,
}

#[pymethods]
impl TimeGridCalendar {
    #[new]
    fn new() -> Self {
        Self {
            inner: Calendar::default(),
        }
    }

    #[getter]
    fn DefaultCapacity(&self) -> i32 {
        self.inner.default_capacity
    }

    #[getter]
    fn IsComposite(&self) -> bool {
        self.inner.is_composite()
    }

    #[getter]
    fn Components(&self) -> Vec<TimeGridCalendar> {
        self.inner
            .components
            .iter()
            .cloned()
            .map(|inner| TimeGridCalendar { inner })
            .collect()
    }

    #[getter]
    fn OpenRules(&self) -> Vec<TimeGridWeeklyRule> {
        self.inner
            .open_rules
            .iter()
            .copied()
            .map(Into::into)
            .collect()
    }

    #[getter]
    fn BreakRules(&self) -> Vec<TimeGridWeeklyRule> {
        self.inner
            .break_rules
            .iter()
            .copied()
            .map(Into::into)
            .collect()
    }

    #[getter]
    fn OpenWindows(&self) -> Vec<TimeWindow> {
        self.inner
            .open_windows
            .iter()
            .copied()
            .map(Into::into)
            .collect()
    }

    #[getter]
    fn ClosedWindows(&self) -> Vec<TimeWindow> {
        self.inner
            .closed_windows
            .iter()
            .copied()
            .map(Into::into)
            .collect()
    }

    #[getter]
    fn CapacityOverrides(&self) -> Vec<TimeGridCapacityWindow> {
        self.inner
            .capacity_overrides
            .iter()
            .copied()
            .map(Into::into)
            .collect()
    }

    #[getter]
    fn Entries(&self) -> Vec<TimeGridEntry> {
        self.inner.entries.iter().cloned().map(Into::into).collect()
    }

    #[staticmethod]
    fn Create() -> Self {
        Self::new()
    }

    #[staticmethod]
    fn Weekdays(start: NaiveTime, end: NaiveTime) -> PyResult<Self> {
        let mut calendar = Calendar::default();
        calendar.add_weekday_open_rule(start, end)?;
        Ok(Self { inner: calendar })
    }

    #[staticmethod]
    fn Window(start: NaiveDateTime, end: NaiveDateTime) -> PyResult<Self> {
        let mut calendar = Calendar::default();
        calendar.add_open_window(start, end)?;
        Ok(Self { inner: calendar })
    }

    #[staticmethod]
    #[pyo3(signature = (*calendars))]
    fn Intersect(calendars: &Bound<'_, PyTuple>) -> PyResult<Self> {
        if calendars.is_empty() {
            return Err(PyValueError::new_err("At least one calendar is required."));
        }
        let mut components = Vec::new();
        for item in calendars.iter() {
            components.push(item.extract::<PyRef<TimeGridCalendar>>()?.inner.clone());
        }
        if components.len() == 1 {
            Ok(Self {
                inner: components.remove(0),
            })
        } else {
            Ok(Self {
                inner: Calendar {
                    components,
                    ..Calendar::default()
                },
            })
        }
    }

    fn And(&self, other: &TimeGridCalendar) -> Self {
        Self {
            inner: Calendar {
                components: vec![self.inner.clone(), other.inner.clone()],
                ..Calendar::default()
            },
        }
    }

    fn At(&self, start: NaiveDateTime) -> TimeGridPointQuery {
        TimeGridPointQuery {
            calendar: self.inner.clone(),
            start,
        }
    }

    fn Between(&self, start: NaiveDateTime, end: NaiveDateTime) -> TimeGridRangeQuery {
        TimeGridRangeQuery {
            calendar: self.inner.clone(),
            start,
            end,
        }
    }

    fn Compile(&self, start: NaiveDateTime, end: NaiveDateTime) -> PyResult<TimeGridTimeline> {
        Window::new(start, end)?;
        Ok(TimeGridTimeline {
            source: self.inner.clone(),
            window: Window { start, end },
            states: self.inner.get_state_windows(start, end)?,
        })
    }

    fn ToDefinition(&self) -> PyResult<TimeGridDefinition> {
        if self.inner.is_composite() {
            return Err(PyRuntimeError::new_err(
                "Composite calendars are query-only and cannot be serialized as one definition. Serialize the source calendars instead.",
            ));
        }
        Ok(TimeGridDefinition {
            inner: Definition::from_calendar(&self.inner),
        })
    }

    #[pyo3(signature = (indented=false))]
    fn ToJson(&self, indented: bool) -> PyResult<String> {
        self.ToDefinition()?.ToJson(indented)
    }

    #[staticmethod]
    fn FromDefinition(definition: &TimeGridDefinition) -> PyResult<Self> {
        Ok(Self {
            inner: definition.inner.to_calendar()?,
        })
    }

    #[staticmethod]
    fn FromJson(json: &str) -> PyResult<Self> {
        Self::FromDefinition(&TimeGridDefinition::FromJson(json)?)
    }

    #[pyo3(signature = (instant, end=None, searchRadius=None))]
    fn Analyze(
        &self,
        py: Python<'_>,
        instant: NaiveDateTime,
        end: Option<&Bound<'_, PyAny>>,
        searchRadius: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        if let Some(value) = end {
            if let Ok(range_end) = value.extract::<NaiveDateTime>() {
                return Ok(Py::new(
                    py,
                    TimeGridTimelineAnalysis {
                        window: Window {
                            start: instant,
                            end: range_end,
                        },
                        segments: self.inner.get_state_windows(instant, range_end)?,
                    },
                )?
                .into_any());
            }
        }
        let radius = match (end, searchRadius) {
            (Some(value), None) => Some(any_to_duration(value)?),
            (_, Some(value)) => Some(any_to_duration(value)?),
            _ => None,
        };
        Ok(Py::new(py, self.instant_analysis(instant, radius)?)?.into_any())
    }

    fn SetDefaultCapacity<'py>(
        mut slf: PyRefMut<'py, Self>,
        capacity: i32,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.set_default_capacity(capacity)?;
        Ok(slf)
    }

    #[pyo3(signature = (*args))]
    fn Capacity<'py>(
        mut slf: PyRefMut<'py, Self>,
        args: &Bound<'py, PyTuple>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        match args.len() {
            1 => slf
                .inner
                .set_default_capacity(args.get_item(0)?.extract()?)?,
            3 => slf.inner.add_capacity_override(
                args.get_item(0)?.extract()?,
                args.get_item(1)?.extract()?,
                args.get_item(2)?.extract()?,
            )?,
            _ => return Err(PyValueError::new_err("Capacity expects 1 or 3 arguments.")),
        }
        Ok(slf)
    }

    fn AddOpenRule<'py>(
        mut slf: PyRefMut<'py, Self>,
        day: u8,
        start: NaiveTime,
        end: NaiveTime,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.add_open_rule(day, start, end)?;
        Ok(slf)
    }

    fn AddWeekdayOpenRule<'py>(
        mut slf: PyRefMut<'py, Self>,
        start: NaiveTime,
        end: NaiveTime,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.add_weekday_open_rule(start, end)?;
        Ok(slf)
    }

    fn OpenWeekdays<'py>(
        slf: PyRefMut<'py, Self>,
        start: NaiveTime,
        end: NaiveTime,
    ) -> PyResult<PyRefMut<'py, Self>> {
        Self::AddWeekdayOpenRule(slf, start, end)
    }

    fn AddBreakRule<'py>(
        mut slf: PyRefMut<'py, Self>,
        day: u8,
        start: NaiveTime,
        end: NaiveTime,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.add_break_rule(day, start, end)?;
        Ok(slf)
    }

    fn AddWeekdayBreakRule<'py>(
        mut slf: PyRefMut<'py, Self>,
        start: NaiveTime,
        end: NaiveTime,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.add_weekday_break_rule(start, end)?;
        Ok(slf)
    }

    fn BreakWeekdays<'py>(
        slf: PyRefMut<'py, Self>,
        start: NaiveTime,
        end: NaiveTime,
    ) -> PyResult<PyRefMut<'py, Self>> {
        Self::AddWeekdayBreakRule(slf, start, end)
    }

    fn AddOpenWindow<'py>(
        mut slf: PyRefMut<'py, Self>,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.add_open_window(start, end)?;
        Ok(slf)
    }

    fn Open<'py>(
        slf: PyRefMut<'py, Self>,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<PyRefMut<'py, Self>> {
        Self::AddOpenWindow(slf, start, end)
    }

    fn AddHoliday<'py>(
        mut slf: PyRefMut<'py, Self>,
        date: NaiveDate,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.add_holiday(date)?;
        Ok(slf)
    }

    #[pyo3(signature = (*args))]
    fn Close<'py>(
        mut slf: PyRefMut<'py, Self>,
        args: &Bound<'py, PyTuple>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        match args.len() {
            1 => slf.inner.add_holiday(args.get_item(0)?.extract()?)?,
            2 => slf
                .inner
                .add_closed_window(args.get_item(0)?.extract()?, args.get_item(1)?.extract()?)?,
            _ => return Err(PyValueError::new_err("Close expects 1 or 2 arguments.")),
        }
        Ok(slf)
    }

    fn AddDowntime<'py>(
        mut slf: PyRefMut<'py, Self>,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.add_closed_window(start, end)?;
        Ok(slf)
    }

    fn Down<'py>(
        slf: PyRefMut<'py, Self>,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<PyRefMut<'py, Self>> {
        Self::AddDowntime(slf, start, end)
    }

    fn AddClosedWindow<'py>(
        slf: PyRefMut<'py, Self>,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<PyRefMut<'py, Self>> {
        Self::AddDowntime(slf, start, end)
    }

    fn AddCapacityOverride<'py>(
        mut slf: PyRefMut<'py, Self>,
        start: NaiveDateTime,
        end: NaiveDateTime,
        capacity: i32,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.add_capacity_override(start, end, capacity)?;
        Ok(slf)
    }

    fn SetOpenWindow<'py>(
        mut slf: PyRefMut<'py, Self>,
        id: String,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.set_entry(id, 0, start, end, 0)?;
        Ok(slf)
    }

    fn SetClosedWindow<'py>(
        mut slf: PyRefMut<'py, Self>,
        id: String,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.set_entry(id, 1, start, end, 0)?;
        Ok(slf)
    }

    fn SetCapacityWindow<'py>(
        mut slf: PyRefMut<'py, Self>,
        id: String,
        start: NaiveDateTime,
        end: NaiveDateTime,
        capacity: i32,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.set_entry(id, 2, start, end, capacity)?;
        Ok(slf)
    }

    fn GetEntry(&self, id: String) -> PyResult<Option<TimeGridEntry>> {
        validate_entry_id(&id)?;
        Ok(self
            .inner
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
            .map(Into::into))
    }

    fn RemoveEntry(mut slf: PyRefMut<'_, Self>, id: String) -> PyResult<bool> {
        slf.inner.ensure_editable()?;
        validate_entry_id(&id)?;
        Ok(
            if let Some(index) = slf.inner.entries.iter().position(|entry| entry.id == id) {
                slf.inner.entries.remove(index);
                true
            } else {
                false
            },
        )
    }

    fn ClearOpenRules<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.ensure_editable()?;
        slf.inner.open_rules.clear();
        Ok(slf)
    }

    fn ClearOpenWindows<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.ensure_editable()?;
        slf.inner.open_windows.clear();
        Ok(slf)
    }

    fn ClearBreakRules<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.ensure_editable()?;
        slf.inner.break_rules.clear();
        Ok(slf)
    }

    fn ClearClosedWindows<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.ensure_editable()?;
        slf.inner.closed_windows.clear();
        Ok(slf)
    }

    fn ClearCapacityOverrides<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.ensure_editable()?;
        slf.inner.capacity_overrides.clear();
        Ok(slf)
    }

    fn ClearEntries<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.ensure_editable()?;
        slf.inner.entries.clear();
        Ok(slf)
    }

    fn Clear<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<PyRefMut<'py, Self>> {
        slf.inner.ensure_editable()?;
        slf.inner = Calendar::default();
        Ok(slf)
    }

    fn CanWork(&self, target: &Bound<'_, PyAny>) -> PyResult<bool> {
        if let Ok(window_ref) = target.extract::<PyRef<TimeWindow>>() {
            let query = window_ref.inner;
            return Ok(query.is_empty()
                || covers(
                    query,
                    &self.inner.get_capacity_windows(query.start, query.end, 1)?,
                ));
        }
        Ok(self.inner.get_capacity_at(target.extract()?) > 0)
    }

    fn HasCapacity(&self, target: &Bound<'_, PyAny>, minimumCapacity: i32) -> PyResult<bool> {
        validate_positive_capacity(minimumCapacity)?;
        if let Ok(window_ref) = target.extract::<PyRef<TimeWindow>>() {
            let query = window_ref.inner;
            return Ok(query.is_empty()
                || covers(
                    query,
                    &self
                        .inner
                        .get_capacity_windows(query.start, query.end, minimumCapacity)?,
                ));
        }
        Ok(self.inner.get_capacity_at(target.extract()?) >= minimumCapacity)
    }

    fn GetCapacityAt(&self, instant: NaiveDateTime) -> i32 {
        self.inner.get_capacity_at(instant)
    }

    #[pyo3(signature = (start, searchUntil=None))]
    fn GetNextOpenTime(
        &self,
        start: NaiveDateTime,
        searchUntil: Option<NaiveDateTime>,
    ) -> PyResult<Option<NaiveDateTime>> {
        self.inner.get_next_open_time(start, searchUntil)
    }

    #[pyo3(signature = (start, duration, searchUntil=None))]
    fn AddWorkDuration(
        &self,
        start: NaiveDateTime,
        duration: &Bound<'_, PyAny>,
        searchUntil: Option<NaiveDateTime>,
    ) -> PyResult<NaiveDateTime> {
        self.inner
            .add_work_duration(start, any_to_duration(duration)?, searchUntil)
    }

    #[pyo3(signature = (start, duration, searchUntil=None))]
    fn TraceWorkDuration(
        &self,
        start: NaiveDateTime,
        duration: &Bound<'_, PyAny>,
        searchUntil: Option<NaiveDateTime>,
    ) -> PyResult<WorkingTimeTrace> {
        self.inner
            .trace_work_duration(start, any_to_duration(duration)?, searchUntil)
    }

    fn GetWorkingDuration<'py>(
        &self,
        py: Python<'py>,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<Bound<'py, PyDelta>> {
        duration_to_py(py, self.inner.get_working_duration(start, end)?)
    }

    fn GetWorkingTicks(&self, start: NaiveDateTime, end: NaiveDateTime) -> PyResult<i64> {
        Ok(duration_to_ticks(
            self.inner.get_working_duration(start, end)?,
        )?)
    }

    fn GetOpenWindows(
        &self,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<Vec<TimeWindow>> {
        Ok(self
            .inner
            .get_open_windows(start, end)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    #[pyo3(signature = (start, end, minimumCapacity=1))]
    fn GetCapacityWindows(
        &self,
        start: NaiveDateTime,
        end: NaiveDateTime,
        minimumCapacity: i32,
    ) -> PyResult<Vec<TimeWindow>> {
        Ok(self
            .inner
            .get_capacity_windows(start, end, minimumCapacity)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    fn GetUnavailableWindows(
        &self,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<Vec<TimeWindow>> {
        Ok(self
            .inner
            .get_unavailable_windows(start, end)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    fn GetStateWindows(
        &self,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<Vec<TimeGridStateWindow>> {
        Ok(self
            .inner
            .get_state_windows(start, end)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    #[pyo3(signature = (instant, searchRadius=None))]
    fn GetWindowsAt(
        &self,
        instant: NaiveDateTime,
        searchRadius: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<TimeGridWindowMatch>> {
        Ok(self
            .inner
            .get_windows_at(instant, maybe_duration(searchRadius)?)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    #[pyo3(signature = (instant, searchRadius=None))]
    fn GetCurrentOpenWindow(
        &self,
        instant: NaiveDateTime,
        searchRadius: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<TimeWindow>> {
        Ok(self
            .inner
            .get_current_open_window(instant, maybe_duration(searchRadius)?)?
            .map(Into::into))
    }

    #[pyo3(signature = (instant, searchRadius=None))]
    fn GetCurrentUnavailableWindow(
        &self,
        instant: NaiveDateTime,
        searchRadius: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<TimeWindow>> {
        Ok(self
            .inner
            .get_current_unavailable_window(instant, maybe_duration(searchRadius)?)?
            .map(Into::into))
    }

    #[pyo3(signature = (instant, searchRadius=None))]
    fn GetNextTransitionTime(
        &self,
        instant: NaiveDateTime,
        searchRadius: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<NaiveDateTime>> {
        self.inner
            .get_next_transition_time(instant, maybe_duration(searchRadius)?)
    }

    #[pyo3(signature = (instant, searchRadius=None))]
    fn GetPreviousTransitionTime(
        &self,
        instant: NaiveDateTime,
        searchRadius: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<NaiveDateTime>> {
        self.inner
            .get_previous_transition_time(instant, maybe_duration(searchRadius)?)
    }

    #[pyo3(signature = (instant, searchRadius=None))]
    fn GetNearestTransitionTime(
        &self,
        instant: NaiveDateTime,
        searchRadius: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<NaiveDateTime>> {
        self.inner
            .get_nearest_transition_time(instant, maybe_duration(searchRadius)?)
    }

    #[pyo3(signature = (start, duration, minimumCapacity=1, searchUntil=None))]
    fn FindFirstSlot(
        &self,
        start: NaiveDateTime,
        duration: &Bound<'_, PyAny>,
        minimumCapacity: i32,
        searchUntil: Option<NaiveDateTime>,
    ) -> PyResult<Option<TimeWindow>> {
        Ok(self
            .inner
            .find_first_slot(
                start,
                any_to_duration(duration)?,
                minimumCapacity,
                searchUntil,
            )?
            .map(Into::into))
    }
}

impl TimeGridCalendar {
    fn instant_analysis(
        &self,
        instant: NaiveDateTime,
        search_radius: Option<Duration>,
    ) -> PyResult<TimeGridInstantAnalysis> {
        let capacity = self.inner.get_capacity_at(instant);
        let current_window = if capacity > 0 {
            self.inner.get_current_open_window(instant, search_radius)?
        } else {
            self.inner
                .get_current_unavailable_window(instant, search_radius)?
        };
        Ok(TimeGridInstantAnalysis {
            instant,
            capacity,
            current_window,
            previous_transition: self
                .inner
                .get_previous_transition_time(instant, search_radius)?,
            next_transition: self
                .inner
                .get_next_transition_time(instant, search_radius)?,
            matches: self.inner.get_windows_at(instant, search_radius)?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StateWindow {
    window: Window,
    capacity: i32,
}

impl StateWindow {
    fn can_work(self) -> bool {
        self.capacity > 0
    }
}

#[pyclass(module = "timegrid")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeGridStateWindow {
    inner: StateWindow,
}

#[pymethods]
impl TimeGridStateWindow {
    #[getter]
    fn Window(&self) -> TimeWindow {
        self.inner.window.into()
    }

    #[getter]
    fn Capacity(&self) -> i32 {
        self.inner.capacity
    }

    #[getter]
    fn CanWork(&self) -> bool {
        self.inner.can_work()
    }

    fn __richcmp__(&self, other: PyRef<TimeGridStateWindow>, op: CompareOp) -> PyResult<bool> {
        match op {
            CompareOp::Eq => Ok(self.inner == other.inner),
            CompareOp::Ne => Ok(self.inner != other.inner),
            _ => Err(PyValueError::new_err(
                "TimeGridStateWindow only supports equality comparison.",
            )),
        }
    }
}

impl From<StateWindow> for TimeGridStateWindow {
    fn from(inner: StateWindow) -> Self {
        Self { inner }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WindowMatch {
    kind: u8,
    window: Window,
    capacity: Option<i32>,
}

#[pyclass(module = "timegrid")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeGridWindowMatch {
    inner: WindowMatch,
}

#[pymethods]
impl TimeGridWindowMatch {
    #[getter]
    fn Kind(&self) -> u8 {
        self.inner.kind
    }

    #[getter]
    fn Window(&self) -> TimeWindow {
        self.inner.window.into()
    }

    #[getter]
    fn Capacity(&self) -> Option<i32> {
        self.inner.capacity
    }

    fn __richcmp__(&self, other: PyRef<TimeGridWindowMatch>, op: CompareOp) -> PyResult<bool> {
        match op {
            CompareOp::Eq => Ok(self.inner == other.inner),
            CompareOp::Ne => Ok(self.inner != other.inner),
            _ => Err(PyValueError::new_err(
                "TimeGridWindowMatch only supports equality comparison.",
            )),
        }
    }
}

impl From<WindowMatch> for TimeGridWindowMatch {
    fn from(inner: WindowMatch) -> Self {
        Self { inner }
    }
}

#[pyclass(module = "timegrid")]
#[derive(Clone, Debug)]
pub struct TimeGridInstantAnalysis {
    instant: NaiveDateTime,
    capacity: i32,
    current_window: Option<Window>,
    previous_transition: Option<NaiveDateTime>,
    next_transition: Option<NaiveDateTime>,
    matches: Vec<WindowMatch>,
}

#[pymethods]
impl TimeGridInstantAnalysis {
    #[getter]
    fn Instant(&self) -> NaiveDateTime {
        self.instant
    }

    #[getter]
    fn Capacity(&self) -> i32 {
        self.capacity
    }

    #[getter]
    fn CurrentWindow(&self) -> Option<TimeWindow> {
        self.current_window.map(Into::into)
    }

    #[getter]
    fn PreviousTransition(&self) -> Option<NaiveDateTime> {
        self.previous_transition
    }

    #[getter]
    fn NextTransition(&self) -> Option<NaiveDateTime> {
        self.next_transition
    }

    #[getter]
    fn Matches(&self) -> Vec<TimeGridWindowMatch> {
        self.matches.iter().copied().map(Into::into).collect()
    }

    #[getter]
    fn CanWork(&self) -> bool {
        self.capacity > 0
    }
}

#[pyclass(module = "timegrid")]
#[derive(Clone, Debug)]
pub struct TimeGridTimelineAnalysis {
    window: Window,
    segments: Vec<StateWindow>,
}

#[pymethods]
impl TimeGridTimelineAnalysis {
    #[getter]
    fn Window(&self) -> TimeWindow {
        self.window.into()
    }

    #[getter]
    fn Segments(&self) -> Vec<TimeGridStateWindow> {
        self.segments.iter().copied().map(Into::into).collect()
    }

    #[getter]
    fn WorkingDuration<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDelta>> {
        duration_to_py(py, self.working_duration())
    }

    #[getter]
    fn WorkingTicks(&self) -> PyResult<i64> {
        duration_to_ticks(self.working_duration())
    }
}

impl TimeGridTimelineAnalysis {
    fn working_duration(&self) -> Duration {
        self.segments
            .iter()
            .filter(|segment| segment.can_work())
            .fold(Duration::zero(), |sum, segment| {
                sum + segment.window.duration()
            })
    }
}

#[derive(Clone, Debug)]
struct WorkingTimeTraceStep {
    window: Window,
    duration: Duration,
}

#[pyclass(module = "timegrid", name = "WorkingTimeTraceStep")]
#[derive(Clone, Debug)]
pub struct PyWorkingTimeTraceStep {
    inner: WorkingTimeTraceStep,
}

#[pymethods]
impl PyWorkingTimeTraceStep {
    #[getter]
    fn Window(&self) -> TimeWindow {
        self.inner.window.into()
    }

    #[getter]
    fn Duration<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDelta>> {
        duration_to_py(py, self.inner.duration)
    }
}

impl From<WorkingTimeTraceStep> for PyWorkingTimeTraceStep {
    fn from(inner: WorkingTimeTraceStep) -> Self {
        Self { inner }
    }
}

#[pyclass(module = "timegrid")]
#[derive(Clone, Debug)]
pub struct WorkingTimeTrace {
    start: NaiveDateTime,
    requested_duration: Duration,
    result: NaiveDateTime,
    steps: Vec<WorkingTimeTraceStep>,
}

#[pymethods]
impl WorkingTimeTrace {
    #[getter]
    fn Start(&self) -> NaiveDateTime {
        self.start
    }

    #[getter]
    fn RequestedDuration<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDelta>> {
        duration_to_py(py, self.requested_duration)
    }

    #[getter]
    fn Result(&self) -> NaiveDateTime {
        self.result
    }

    #[getter]
    fn Steps(&self) -> Vec<PyWorkingTimeTraceStep> {
        self.steps.iter().cloned().map(Into::into).collect()
    }

    #[getter]
    fn RequestedTicks(&self) -> PyResult<i64> {
        duration_to_ticks(self.requested_duration)
    }

    #[getter]
    fn ConsumedTicks(&self) -> PyResult<i64> {
        duration_to_ticks(
            self.steps
                .iter()
                .fold(Duration::zero(), |sum, step| sum + step.duration),
        )
    }
}

#[pyclass(module = "timegrid")]
#[derive(Clone, Debug)]
pub struct TimeGridPointQuery {
    calendar: Calendar,
    start: NaiveDateTime,
}

#[pymethods]
impl TimeGridPointQuery {
    fn CanWork(&self) -> bool {
        self.calendar.get_capacity_at(self.start) > 0
    }

    fn HasCapacity(&self, minimumCapacity: i32) -> PyResult<bool> {
        validate_positive_capacity(minimumCapacity)?;
        Ok(self.calendar.get_capacity_at(self.start) >= minimumCapacity)
    }

    fn GetCapacity(&self) -> i32 {
        self.calendar.get_capacity_at(self.start)
    }

    #[pyo3(signature = (searchRadius=None))]
    fn Analyze(
        &self,
        searchRadius: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<TimeGridInstantAnalysis> {
        let calendar = TimeGridCalendar {
            inner: self.calendar.clone(),
        };
        calendar.instant_analysis(self.start, maybe_duration(searchRadius)?)
    }

    #[pyo3(signature = (searchRadius=None))]
    fn GetWindows(
        &self,
        searchRadius: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<TimeGridWindowMatch>> {
        Ok(self
            .calendar
            .get_windows_at(self.start, maybe_duration(searchRadius)?)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    #[pyo3(signature = (searchRadius=None))]
    fn GetCurrentOpenWindow(
        &self,
        searchRadius: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<TimeWindow>> {
        Ok(self
            .calendar
            .get_current_open_window(self.start, maybe_duration(searchRadius)?)?
            .map(Into::into))
    }

    #[pyo3(signature = (searchRadius=None))]
    fn GetCurrentUnavailableWindow(
        &self,
        searchRadius: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<TimeWindow>> {
        Ok(self
            .calendar
            .get_current_unavailable_window(self.start, maybe_duration(searchRadius)?)?
            .map(Into::into))
    }

    #[pyo3(signature = (searchRadius=None))]
    fn GetNextTransitionTime(
        &self,
        searchRadius: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<NaiveDateTime>> {
        self.calendar
            .get_next_transition_time(self.start, maybe_duration(searchRadius)?)
    }

    #[pyo3(signature = (searchRadius=None))]
    fn GetPreviousTransitionTime(
        &self,
        searchRadius: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<NaiveDateTime>> {
        self.calendar
            .get_previous_transition_time(self.start, maybe_duration(searchRadius)?)
    }

    #[pyo3(signature = (searchRadius=None))]
    fn GetNearestTransitionTime(
        &self,
        searchRadius: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<NaiveDateTime>> {
        self.calendar
            .get_nearest_transition_time(self.start, maybe_duration(searchRadius)?)
    }

    #[pyo3(signature = (searchUntil=None))]
    fn GetNextOpenTime(
        &self,
        searchUntil: Option<NaiveDateTime>,
    ) -> PyResult<Option<NaiveDateTime>> {
        self.calendar.get_next_open_time(self.start, searchUntil)
    }

    #[pyo3(signature = (duration, searchUntil=None))]
    fn AddWorkDuration(
        &self,
        duration: &Bound<'_, PyAny>,
        searchUntil: Option<NaiveDateTime>,
    ) -> PyResult<NaiveDateTime> {
        self.calendar
            .add_work_duration(self.start, any_to_duration(duration)?, searchUntil)
    }

    #[pyo3(signature = (duration, searchUntil=None))]
    fn TraceWorkDuration(
        &self,
        duration: &Bound<'_, PyAny>,
        searchUntil: Option<NaiveDateTime>,
    ) -> PyResult<WorkingTimeTrace> {
        self.calendar
            .trace_work_duration(self.start, any_to_duration(duration)?, searchUntil)
    }
}

#[pyclass(module = "timegrid")]
#[derive(Clone, Debug)]
pub struct TimeGridRangeQuery {
    calendar: Calendar,
    start: NaiveDateTime,
    end: NaiveDateTime,
}

#[pymethods]
impl TimeGridRangeQuery {
    fn GetOpenWindows(&self) -> PyResult<Vec<TimeWindow>> {
        Ok(self
            .calendar
            .get_open_windows(self.start, self.end)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    fn GetUnavailableWindows(&self) -> PyResult<Vec<TimeWindow>> {
        Ok(self
            .calendar
            .get_unavailable_windows(self.start, self.end)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    fn GetStateWindows(&self) -> PyResult<Vec<TimeGridStateWindow>> {
        Ok(self
            .calendar
            .get_state_windows(self.start, self.end)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    fn Analyze(&self) -> PyResult<TimeGridTimelineAnalysis> {
        Ok(TimeGridTimelineAnalysis {
            window: Window {
                start: self.start,
                end: self.end,
            },
            segments: self.calendar.get_state_windows(self.start, self.end)?,
        })
    }

    fn GetWorkingDuration<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDelta>> {
        duration_to_py(
            py,
            self.calendar.get_working_duration(self.start, self.end)?,
        )
    }

    fn GetWorkingTicks(&self) -> PyResult<i64> {
        duration_to_ticks(self.calendar.get_working_duration(self.start, self.end)?)
    }

    #[pyo3(signature = (minimumCapacity=1))]
    fn GetCapacityWindows(&self, minimumCapacity: i32) -> PyResult<Vec<TimeWindow>> {
        Ok(self
            .calendar
            .get_capacity_windows(self.start, self.end, minimumCapacity)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    fn CanWork(&self) -> PyResult<bool> {
        let query = Window::new(self.start, self.end)?;
        Ok(query.is_empty()
            || covers(
                query,
                &self
                    .calendar
                    .get_capacity_windows(self.start, self.end, 1)?,
            ))
    }

    fn HasCapacity(&self, minimumCapacity: i32) -> PyResult<bool> {
        validate_positive_capacity(minimumCapacity)?;
        let query = Window::new(self.start, self.end)?;
        Ok(query.is_empty()
            || covers(
                query,
                &self
                    .calendar
                    .get_capacity_windows(self.start, self.end, minimumCapacity)?,
            ))
    }

    #[pyo3(signature = (duration, minimumCapacity=1))]
    fn FindFirstSlot(
        &self,
        duration: &Bound<'_, PyAny>,
        minimumCapacity: i32,
    ) -> PyResult<Option<TimeWindow>> {
        Ok(self
            .calendar
            .find_first_slot(
                self.start,
                any_to_duration(duration)?,
                minimumCapacity,
                Some(self.end),
            )?
            .map(Into::into))
    }
}

#[pyclass(module = "timegrid")]
#[derive(Clone, Debug)]
pub struct TimeGridTimeline {
    source: Calendar,
    window: Window,
    states: Vec<StateWindow>,
}

#[pymethods]
impl TimeGridTimeline {
    #[getter]
    fn Window(&self) -> TimeWindow {
        self.window.into()
    }

    fn GetCapacityAt(&self, instant: NaiveDateTime) -> PyResult<i32> {
        Ok(self.state_at(instant)?.capacity)
    }

    fn GetCapacitiesAt(&self, instants: Vec<NaiveDateTime>) -> PyResult<Vec<i32>> {
        instants
            .into_iter()
            .map(|instant| Ok(self.state_at(instant)?.capacity))
            .collect()
    }

    fn CanWork(&self, instant: NaiveDateTime) -> PyResult<bool> {
        Ok(self.GetCapacityAt(instant)? > 0)
    }

    #[pyo3(signature = (instant, second=None, includeMatches=false))]
    fn Analyze(
        &self,
        py: Python<'_>,
        instant: NaiveDateTime,
        second: Option<&Bound<'_, PyAny>>,
        includeMatches: bool,
    ) -> PyResult<Py<PyAny>> {
        if let Some(value) = second {
            if let Ok(end) = value.extract::<NaiveDateTime>() {
                return Ok(Py::new(py, self.AnalyzeRange(instant, end)?)?.into_any());
            }
            if let Ok(include) = value.extract::<bool>() {
                return timeline_instant_analysis(self, py, instant, include);
            }
        }
        timeline_instant_analysis(self, py, instant, includeMatches)
    }

    #[pyo3(signature = (instants, includeMatches=false))]
    fn AnalyzeMany(
        &self,
        instants: Vec<NaiveDateTime>,
        includeMatches: bool,
    ) -> PyResult<Vec<TimeGridInstantAnalysis>> {
        instants
            .into_iter()
            .map(|instant| self.instant_analysis(instant, includeMatches))
            .collect()
    }

    fn AnalyzeRange(
        &self,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<TimeGridTimelineAnalysis> {
        Ok(TimeGridTimelineAnalysis {
            window: Window::new(start, end)?,
            segments: self.get_state_windows(start, end)?,
        })
    }

    fn GetStateWindows(
        &self,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<Vec<TimeGridStateWindow>> {
        Ok(self
            .get_state_windows(start, end)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    fn GetOpenWindows(
        &self,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<Vec<TimeWindow>> {
        Ok(self
            .windows_where(start, end, |state| state.capacity > 0)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    fn GetUnavailableWindows(
        &self,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<Vec<TimeWindow>> {
        Ok(self
            .windows_where(start, end, |state| state.capacity == 0)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    #[pyo3(signature = (start, end, minimumCapacity=1))]
    fn GetCapacityWindows(
        &self,
        start: NaiveDateTime,
        end: NaiveDateTime,
        minimumCapacity: i32,
    ) -> PyResult<Vec<TimeWindow>> {
        validate_positive_capacity(minimumCapacity)?;
        Ok(self
            .windows_where(start, end, |state| state.capacity >= minimumCapacity)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    fn GetWorkingDuration<'py>(
        &self,
        py: Python<'py>,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<Bound<'py, PyDelta>> {
        let total = self
            .get_state_windows(start, end)?
            .iter()
            .filter(|state| state.can_work())
            .fold(Duration::zero(), |sum, state| sum + state.window.duration());
        duration_to_py(py, total)
    }

    #[pyo3(signature = (start, duration, minimumCapacity=1, searchUntil=None))]
    fn FindFirstSlot(
        &self,
        start: NaiveDateTime,
        duration: &Bound<'_, PyAny>,
        minimumCapacity: i32,
        searchUntil: Option<NaiveDateTime>,
    ) -> PyResult<Option<TimeWindow>> {
        let duration = any_to_duration(duration)?;
        if duration < Duration::zero() {
            return Err(PyValueError::new_err("Duration cannot be negative."));
        }
        let end = searchUntil.unwrap_or(self.window.end);
        for window in self.windows_where(start, end, |state| state.capacity >= minimumCapacity)? {
            if duration == Duration::zero() {
                return Ok(Some(
                    Window {
                        start: window.start,
                        end: window.start,
                    }
                    .into(),
                ));
            }
            if window.duration() >= duration {
                return Ok(Some(
                    Window {
                        start: window.start,
                        end: window.start + duration,
                    }
                    .into(),
                ));
            }
        }
        Ok(None)
    }
}

impl TimeGridTimeline {
    fn state_at(&self, instant: NaiveDateTime) -> PyResult<StateWindow> {
        Ok(self.states[self.state_index_at(instant)?])
    }

    fn instant_analysis(
        &self,
        instant: NaiveDateTime,
        include_matches: bool,
    ) -> PyResult<TimeGridInstantAnalysis> {
        let index = self.state_index_at(instant)?;
        let state = self.states[index];
        Ok(TimeGridInstantAnalysis {
            instant,
            capacity: state.capacity,
            current_window: Some(state.window),
            previous_transition: self.previous_transition(index, instant),
            next_transition: self.next_transition(index, instant),
            matches: if include_matches {
                self.source.get_windows_at(instant, None)?
            } else {
                Vec::new()
            },
        })
    }

    fn state_index_at(&self, instant: NaiveDateTime) -> PyResult<usize> {
        if instant < self.window.start || instant >= self.window.end {
            return Err(PyValueError::new_err(
                "Timestamp is outside the compiled timeline.",
            ));
        }
        let index = self.first_state_ending_after(instant);
        if index >= self.states.len() || !self.states[index].window.contains(instant) {
            return Err(PyValueError::new_err(
                "Timestamp is outside the compiled timeline.",
            ));
        }
        Ok(index)
    }

    fn get_state_windows(
        &self,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> PyResult<Vec<StateWindow>> {
        self.ensure_range(start, end)?;
        if start == end {
            return Ok(Vec::new());
        }
        let mut result = Vec::new();
        let mut index = self.first_state_ending_after(start);
        while index < self.states.len() && self.states[index].window.start < end {
            if let Some(window) = self.states[index].window.intersect(Window { start, end }) {
                add_state_window(
                    &mut result,
                    StateWindow {
                        window,
                        capacity: self.states[index].capacity,
                    },
                );
            }
            index += 1;
        }
        Ok(result)
    }

    fn windows_where<F>(
        &self,
        start: NaiveDateTime,
        end: NaiveDateTime,
        predicate: F,
    ) -> PyResult<Vec<Window>>
    where
        F: Fn(StateWindow) -> bool,
    {
        Ok(merge(
            self.get_state_windows(start, end)?
                .into_iter()
                .filter(|state| predicate(*state))
                .map(|state| state.window)
                .collect(),
        ))
    }

    fn first_state_ending_after(&self, value: NaiveDateTime) -> usize {
        let mut low = 0usize;
        let mut high = self.states.len();
        while low < high {
            let mid = low + ((high - low) / 2);
            if self.states[mid].window.end <= value {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        low
    }

    fn previous_transition(
        &self,
        state_index: usize,
        instant: NaiveDateTime,
    ) -> Option<NaiveDateTime> {
        (0..=state_index).rev().find_map(|i| {
            let candidate = self.states[i].window.start;
            (candidate != self.window.start && candidate < instant).then_some(candidate)
        })
    }

    fn next_transition(&self, state_index: usize, instant: NaiveDateTime) -> Option<NaiveDateTime> {
        (state_index..self.states.len()).find_map(|i| {
            let candidate = self.states[i].window.end;
            (candidate != self.window.end && candidate > instant).then_some(candidate)
        })
    }

    fn ensure_range(&self, start: NaiveDateTime, end: NaiveDateTime) -> PyResult<()> {
        Window::new(start, end)?;
        if start < self.window.start || end > self.window.end {
            return Err(PyValueError::new_err(
                "Range is outside the compiled timeline.",
            ));
        }
        Ok(())
    }
}

fn timeline_instant_analysis(
    timeline: &TimeGridTimeline,
    py: Python<'_>,
    instant: NaiveDateTime,
    include_matches: bool,
) -> PyResult<Py<PyAny>> {
    Ok(Py::new(py, timeline.instant_analysis(instant, include_matches)?)?.into_any())
}

#[pyclass(module = "timegrid")]
#[derive(Clone, Debug)]
pub struct TimeGridTimelineBatch {
    timelines: Vec<TimeGridTimeline>,
}

#[pymethods]
impl TimeGridTimelineBatch {
    #[new]
    fn new(timelines: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut result = Vec::new();
        for item in timelines.try_iter()? {
            let timeline = item?.extract::<PyRef<TimeGridTimeline>>()?;
            result.push((*timeline).clone());
        }
        Ok(Self { timelines: result })
    }

    #[staticmethod]
    fn Create(timelines: &Bound<'_, PyAny>) -> PyResult<Self> {
        Self::new(timelines)
    }

    #[getter]
    fn Count(&self) -> usize {
        self.timelines.len()
    }

    fn GetCapacitiesAt(&self, instant: NaiveDateTime) -> PyResult<Vec<i32>> {
        self.timelines
            .iter()
            .map(|timeline| timeline.GetCapacityAt(instant))
            .collect()
    }

    #[pyo3(signature = (instant, includeMatches=false))]
    fn Analyze(
        &self,
        instant: NaiveDateTime,
        includeMatches: bool,
    ) -> PyResult<Vec<TimeGridInstantAnalysis>> {
        self.timelines
            .iter()
            .map(|timeline| timeline.instant_analysis(instant, includeMatches))
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
struct CapacitySource {
    window: Window,
    capacity: i32,
    order: usize,
}

#[derive(Clone, Copy, Debug)]
struct CapacityEvent {
    time: NaiveDateTime,
    order: usize,
    is_start: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Definition {
    default_capacity: i32,
    open_rules: Vec<RuleDefinition>,
    break_rules: Vec<RuleDefinition>,
    open_windows: Vec<WindowDefinition>,
    closed_windows: Vec<WindowDefinition>,
    capacity_windows: Vec<CapacityDefinition>,
    entries: Vec<EntryDefinition>,
}

impl Definition {
    fn from_calendar(calendar: &Calendar) -> Self {
        Self {
            default_capacity: calendar.default_capacity,
            open_rules: calendar
                .open_rules
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
            break_rules: calendar
                .break_rules
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
            open_windows: calendar
                .open_windows
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
            closed_windows: calendar
                .closed_windows
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
            capacity_windows: calendar
                .capacity_overrides
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
            entries: calendar.entries.iter().cloned().map(Into::into).collect(),
        }
    }

    fn to_calendar(&self) -> PyResult<Calendar> {
        let mut calendar = Calendar::default();
        calendar.set_default_capacity(self.default_capacity)?;
        for rule in &self.open_rules {
            calendar.add_open_rule(rule.day, rule.start, rule.end)?;
        }
        for rule in &self.break_rules {
            calendar.add_break_rule(rule.day, rule.start, rule.end)?;
        }
        for window in &self.open_windows {
            calendar.add_open_window(window.start, window.end)?;
        }
        for window in &self.closed_windows {
            calendar.add_closed_window(window.start, window.end)?;
        }
        for capacity in &self.capacity_windows {
            calendar.add_capacity_override(capacity.start, capacity.end, capacity.capacity)?;
        }
        for entry in &self.entries {
            calendar.set_entry(
                entry.id.clone(),
                entry.kind,
                entry.start,
                entry.end,
                entry.capacity,
            )?;
        }
        Ok(calendar)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuleDefinition {
    day: u8,
    start: NaiveTime,
    end: NaiveTime,
}

impl From<WeeklyRule> for RuleDefinition {
    fn from(rule: WeeklyRule) -> Self {
        Self {
            day: rule.day,
            start: rule.start,
            end: rule.end,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowDefinition {
    start: NaiveDateTime,
    end: NaiveDateTime,
}

impl From<Window> for WindowDefinition {
    fn from(window: Window) -> Self {
        Self {
            start: window.start,
            end: window.end,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CapacityDefinition {
    start: NaiveDateTime,
    end: NaiveDateTime,
    capacity: i32,
}

impl From<CapacityWindow> for CapacityDefinition {
    fn from(window: CapacityWindow) -> Self {
        Self {
            start: window.window.start,
            end: window.window.end,
            capacity: window.capacity,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntryDefinition {
    id: String,
    kind: u8,
    start: NaiveDateTime,
    end: NaiveDateTime,
    capacity: i32,
}

impl From<Entry> for EntryDefinition {
    fn from(entry: Entry) -> Self {
        Self {
            id: entry.id,
            kind: entry.kind,
            start: entry.window.start,
            end: entry.window.end,
            capacity: entry.capacity,
        }
    }
}

#[pyclass(module = "timegrid")]
#[derive(Clone, Debug)]
pub struct TimeGridDefinition {
    inner: Definition,
}

#[pymethods]
impl TimeGridDefinition {
    fn ToCalendar(&self) -> PyResult<TimeGridCalendar> {
        Ok(TimeGridCalendar {
            inner: self.inner.to_calendar()?,
        })
    }

    #[pyo3(signature = (indented=false))]
    fn ToJson(&self, indented: bool) -> PyResult<String> {
        if indented {
            serde_json::to_string_pretty(&self.inner)
                .map_err(|err| PyValueError::new_err(err.to_string()))
        } else {
            serde_json::to_string(&self.inner).map_err(|err| PyValueError::new_err(err.to_string()))
        }
    }

    #[staticmethod]
    fn FromJson(json: &str) -> PyResult<Self> {
        if json.trim().is_empty() {
            return Err(PyValueError::new_err("JSON cannot be empty."));
        }
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|err| PyValueError::new_err(err.to_string()))?,
        })
    }
}

fn expand_rules(rules: &[WeeklyRule], query: Window) -> Vec<Window> {
    let mut result = Vec::new();
    let mut date = query
        .start
        .date()
        .checked_sub_days(Days::new(1))
        .unwrap_or_else(|| query.start.date());
    let last_date = query.end.date();
    while date <= last_date {
        for rule in rules {
            if day_of_week(date) == rule.day {
                if let Some(window) = rule.to_window(date).intersect(query) {
                    result.push(window);
                }
            }
        }
        match date.checked_add_days(Days::new(1)) {
            Some(next) => date = next,
            None => break,
        }
    }
    result.sort_by(compare_windows);
    result
}

fn add_clipped_windows(source: &[Window], query: Window, result: &mut Vec<Window>) {
    result.extend(source.iter().filter_map(|window| window.intersect(query)));
}

fn subtract_into(window: Window, closed: Window, result: &mut Vec<Window>) {
    if !window.overlaps(closed) {
        result.push(window);
        return;
    }
    if window.start < closed.start {
        let end = window.end.min(closed.start);
        if window.start < end {
            result.push(Window {
                start: window.start,
                end,
            });
        }
    }
    if closed.end < window.end {
        let start = window.start.max(closed.end);
        if start < window.end {
            result.push(Window {
                start,
                end: window.end,
            });
        }
    }
}

fn complement(query: Window, open_windows: &[Window]) -> Vec<Window> {
    let mut result = Vec::new();
    let mut cursor = query.start;
    for window in open_windows {
        if window.start > cursor {
            result.push(Window {
                start: cursor,
                end: window.start,
            });
        }
        if window.end > cursor {
            cursor = window.end;
        }
    }
    if cursor < query.end {
        result.push(Window {
            start: cursor,
            end: query.end,
        });
    }
    result
}

fn covers(query: Window, windows: &[Window]) -> bool {
    let mut cursor = query.start;
    for window in windows {
        if window.start > cursor {
            return false;
        }
        if window.end > cursor {
            cursor = window.end;
            if cursor >= query.end {
                return true;
            }
        }
    }
    cursor >= query.end
}

fn add_state_window(result: &mut Vec<StateWindow>, next: StateWindow) {
    if next.window.is_empty() {
        return;
    }
    if let Some(previous) = result.last_mut() {
        if previous.capacity == next.capacity && previous.window.end == next.window.start {
            previous.window.end = next.window.end;
            return;
        }
    }
    result.push(next);
}

fn merge(mut windows: Vec<Window>) -> Vec<Window> {
    windows.retain(|window| !window.is_empty());
    windows.sort_by(compare_windows);
    let mut merged: Vec<Window> = Vec::new();
    for window in windows {
        if let Some(previous) = merged.last_mut() {
            if window.start <= previous.end {
                previous.end = previous.end.max(window.end);
                continue;
            }
        }
        merged.push(window);
    }
    merged
}

fn intersect_windows(left: &[Window], right: &[Window]) -> Vec<Window> {
    let mut result = Vec::new();
    let mut left_index = 0usize;
    let mut right_index = 0usize;
    while left_index < left.len() && right_index < right.len() {
        let a = left[left_index];
        let b = right[right_index];
        if let Some(window) = a.intersect(b) {
            result.push(window);
        }
        if a.end <= b.end {
            left_index += 1;
        } else {
            right_index += 1;
        }
    }
    merge(result)
}

fn add_rule_matches(
    rules: &[WeeklyRule],
    kind: u8,
    instant: NaiveDateTime,
    result: &mut Vec<WindowMatch>,
) {
    let mut date = instant
        .date()
        .checked_sub_days(Days::new(1))
        .unwrap_or_else(|| instant.date());
    let last_date = instant.date();
    while date <= last_date {
        for rule in rules {
            if day_of_week(date) == rule.day {
                let window = rule.to_window(date);
                if window.contains(instant) {
                    result.push(WindowMatch {
                        kind,
                        window,
                        capacity: None,
                    });
                }
            }
        }
        match date.checked_add_days(Days::new(1)) {
            Some(next) => date = next,
            None => break,
        }
    }
}

fn compare_windows(left: &Window, right: &Window) -> std::cmp::Ordering {
    left.start
        .cmp(&right.start)
        .then_with(|| left.end.cmp(&right.end))
}

fn search_window_around(
    instant: NaiveDateTime,
    search_radius: Option<Duration>,
) -> PyResult<Window> {
    let radius = search_radius.unwrap_or_else(|| Duration::days(DEFAULT_SEARCH_DAYS));
    if radius <= Duration::zero() {
        return Err(PyValueError::new_err("Search radius must be positive."));
    }
    Ok(Window {
        start: instant - radius,
        end: instant + radius,
    })
}

fn search_chunks(
    start: NaiveDateTime,
    until: NaiveDateTime,
    minimum_span: Duration,
) -> Vec<Window> {
    let chunk_size = if minimum_span > Duration::days(DEFAULT_SEARCH_DAYS) {
        minimum_span + Duration::days(DEFAULT_SEARCH_DAYS)
    } else {
        Duration::days(DEFAULT_SEARCH_DAYS)
    };
    let mut chunks = Vec::new();
    let mut cursor = start;
    while cursor < until {
        let mut end = cursor + chunk_size;
        if end > until {
            end = until;
        }
        chunks.push(Window { start: cursor, end });
        let next = if minimum_span > Duration::zero() && end < until {
            end - minimum_span
        } else {
            end
        };
        cursor = if next <= cursor { end } else { next };
    }
    chunks
}

fn day_of_week(date: NaiveDate) -> u8 {
    date.weekday().num_days_from_sunday() as u8
}

fn next_day(day: u8) -> u8 {
    (day + 1) % 7
}

fn min_dt(current: Option<NaiveDateTime>, candidate: NaiveDateTime) -> NaiveDateTime {
    current
        .map(|value| value.min(candidate))
        .unwrap_or(candidate)
}

fn max_dt(current: Option<NaiveDateTime>, candidate: NaiveDateTime) -> NaiveDateTime {
    current
        .map(|value| value.max(candidate))
        .unwrap_or(candidate)
}

fn validate_positive_capacity(minimum_capacity: i32) -> PyResult<()> {
    if minimum_capacity <= 0 {
        Err(PyValueError::new_err("Required capacity must be positive."))
    } else {
        Ok(())
    }
}

fn validate_entry_id(id: &str) -> PyResult<()> {
    if id.trim().is_empty() {
        Err(PyValueError::new_err("Entry id cannot be empty."))
    } else {
        Ok(())
    }
}

fn any_to_duration(any: &Bound<'_, PyAny>) -> PyResult<Duration> {
    if let Ok(duration) = any.extract::<Duration>() {
        return Ok(duration);
    }
    let days: i64 = any.getattr("days")?.extract()?;
    let seconds: i64 = any.getattr("seconds")?.extract()?;
    let microseconds: i64 = any.getattr("microseconds")?.extract()?;
    Ok(Duration::days(days) + Duration::seconds(seconds) + Duration::microseconds(microseconds))
}

fn maybe_duration(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<Duration>> {
    value.map(any_to_duration).transpose()
}

fn duration_to_py<'py>(py: Python<'py>, duration: Duration) -> PyResult<Bound<'py, PyDelta>> {
    let total_microseconds = duration
        .num_microseconds()
        .ok_or_else(|| PyOverflowError::new_err("Duration is outside Python timedelta range."))?;
    let day_microseconds = 86_400_000_000i64;
    let days = total_microseconds.div_euclid(day_microseconds);
    let day_remainder = total_microseconds.rem_euclid(day_microseconds);
    let seconds = day_remainder / 1_000_000;
    let microseconds = day_remainder % 1_000_000;
    PyDelta::new(
        py,
        days.try_into()
            .map_err(|_| PyOverflowError::new_err("Duration day overflow."))?,
        seconds.try_into().unwrap(),
        microseconds.try_into().unwrap(),
        true,
    )
}

fn duration_to_ticks(duration: Duration) -> PyResult<i64> {
    duration
        .num_microseconds()
        .and_then(|value| value.checked_mul(TICKS_PER_MICROSECOND))
        .ok_or_else(|| PyOverflowError::new_err("Duration tick overflow."))
}

#[pyfunction]
fn Hours<'py>(py: Python<'py>, value: i64) -> PyResult<Bound<'py, PyDelta>> {
    duration_to_py(py, Duration::hours(value))
}

#[pyfunction]
fn Minutes<'py>(py: Python<'py>, value: i64) -> PyResult<Bound<'py, PyDelta>> {
    duration_to_py(py, Duration::minutes(value))
}

#[pyfunction]
fn Seconds<'py>(py: Python<'py>, value: i64) -> PyResult<Bound<'py, PyDelta>> {
    duration_to_py(py, Duration::seconds(value))
}

#[pymodule]
fn timegrid(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_class::<DayOfWeek>()?;
    m.add_class::<TimeGridEntryKind>()?;
    m.add_class::<TimeGridWindowKind>()?;
    m.add_class::<TimeWindow>()?;
    m.add_class::<TimeGridWeeklyRule>()?;
    m.add_class::<TimeGridCapacityWindow>()?;
    m.add_class::<TimeGridEntry>()?;
    m.add_class::<TimeGridCalendar>()?;
    m.add_class::<TimeGridDefinition>()?;
    m.add_class::<TimeGridPointQuery>()?;
    m.add_class::<TimeGridRangeQuery>()?;
    m.add_class::<TimeGridTimeline>()?;
    m.add_class::<TimeGridTimelineBatch>()?;
    m.add_class::<TimeGridInstantAnalysis>()?;
    m.add_class::<TimeGridTimelineAnalysis>()?;
    m.add_class::<TimeGridStateWindow>()?;
    m.add_class::<TimeGridWindowMatch>()?;
    m.add_class::<WorkingTimeTrace>()?;
    m.add_class::<PyWorkingTimeTraceStep>()?;
    m.add_function(wrap_pyfunction!(Hours, m)?)?;
    m.add_function(wrap_pyfunction!(Minutes, m)?)?;
    m.add_function(wrap_pyfunction!(Seconds, m)?)?;
    m.add_function(wrap_pyfunction!(hours, m)?)?;
    m.add_function(wrap_pyfunction!(minutes, m)?)?;
    m.add_function(wrap_pyfunction!(seconds, m)?)?;
    add_pythonic_aliases(m)?;
    Ok(())
}

#[pyfunction]
fn hours<'py>(py: Python<'py>, value: i64) -> PyResult<Bound<'py, PyDelta>> {
    Hours(py, value)
}

#[pyfunction]
fn minutes<'py>(py: Python<'py>, value: i64) -> PyResult<Bound<'py, PyDelta>> {
    Minutes(py, value)
}

#[pyfunction]
fn seconds<'py>(py: Python<'py>, value: i64) -> PyResult<Bound<'py, PyDelta>> {
    Seconds(py, value)
}

fn add_pythonic_aliases(m: &Bound<'_, PyModule>) -> PyResult<()> {
    alias_attrs(
        m,
        "TimeWindow",
        &[
            ("Start", "start"),
            ("End", "end"),
            ("Duration", "duration"),
            ("IsEmpty", "is_empty"),
            ("Contains", "contains"),
            ("Overlaps", "overlaps"),
            ("Intersect", "intersect"),
        ],
    )?;
    alias_attrs(
        m,
        "TimeGridWeeklyRule",
        &[
            ("Day", "day"),
            ("Start", "start"),
            ("End", "end"),
            ("Contains", "contains"),
            ("ToWindow", "to_window"),
        ],
    )?;
    alias_attrs(
        m,
        "TimeGridCapacityWindow",
        &[("Window", "window"), ("Capacity", "capacity")],
    )?;
    alias_attrs(
        m,
        "DayOfWeek",
        &[
            ("Sunday", "SUNDAY"),
            ("Monday", "MONDAY"),
            ("Tuesday", "TUESDAY"),
            ("Wednesday", "WEDNESDAY"),
            ("Thursday", "THURSDAY"),
            ("Friday", "FRIDAY"),
            ("Saturday", "SATURDAY"),
        ],
    )?;
    alias_attrs(
        m,
        "TimeGridEntryKind",
        &[
            ("Open", "OPEN"),
            ("Closed", "CLOSED"),
            ("Capacity", "CAPACITY"),
        ],
    )?;
    alias_attrs(
        m,
        "TimeGridWindowKind",
        &[
            ("Available", "AVAILABLE"),
            ("Unavailable", "UNAVAILABLE"),
            ("OpenRule", "OPEN_RULE"),
            ("OpenWindow", "OPEN_WINDOW"),
            ("BreakRule", "BREAK_RULE"),
            ("ClosedWindow", "CLOSED_WINDOW"),
            ("CapacityOverride", "CAPACITY_OVERRIDE"),
        ],
    )?;
    alias_attrs(
        m,
        "TimeGridEntry",
        &[
            ("Id", "id"),
            ("Kind", "kind"),
            ("Window", "window"),
            ("Capacity", "capacity"),
        ],
    )?;
    alias_attrs(
        m,
        "TimeGridCalendar",
        &[
            ("DefaultCapacity", "default_capacity"),
            ("IsComposite", "is_composite"),
            ("Components", "components"),
            ("OpenRules", "open_rules"),
            ("BreakRules", "break_rules"),
            ("OpenWindows", "open_windows"),
            ("ClosedWindows", "closed_windows"),
            ("CapacityOverrides", "capacity_overrides"),
            ("Entries", "entries"),
            ("Create", "create"),
            ("Weekdays", "weekdays"),
            ("Window", "window"),
            ("Intersect", "intersect"),
            ("And", "and_"),
            ("At", "at"),
            ("Between", "between"),
            ("Compile", "compile"),
            ("ToDefinition", "to_definition"),
            ("ToJson", "to_json"),
            ("FromDefinition", "from_definition"),
            ("FromJson", "from_json"),
            ("Analyze", "analyze"),
            ("SetDefaultCapacity", "set_default_capacity"),
            ("Capacity", "capacity"),
            ("AddOpenRule", "add_open_rule"),
            ("AddWeekdayOpenRule", "add_weekday_open_rule"),
            ("OpenWeekdays", "open_weekdays"),
            ("AddBreakRule", "add_break_rule"),
            ("AddWeekdayBreakRule", "add_weekday_break_rule"),
            ("BreakWeekdays", "break_weekdays"),
            ("AddOpenWindow", "add_open_window"),
            ("Open", "open"),
            ("AddHoliday", "add_holiday"),
            ("Close", "close"),
            ("AddDowntime", "add_downtime"),
            ("Down", "down"),
            ("AddClosedWindow", "add_closed_window"),
            ("AddCapacityOverride", "add_capacity_override"),
            ("SetOpenWindow", "set_open_window"),
            ("SetClosedWindow", "set_closed_window"),
            ("SetCapacityWindow", "set_capacity_window"),
            ("GetEntry", "get_entry"),
            ("RemoveEntry", "remove_entry"),
            ("ClearOpenRules", "clear_open_rules"),
            ("ClearOpenWindows", "clear_open_windows"),
            ("ClearBreakRules", "clear_break_rules"),
            ("ClearClosedWindows", "clear_closed_windows"),
            ("ClearCapacityOverrides", "clear_capacity_overrides"),
            ("ClearEntries", "clear_entries"),
            ("Clear", "clear"),
            ("CanWork", "can_work"),
            ("HasCapacity", "has_capacity"),
            ("GetCapacityAt", "get_capacity_at"),
            ("GetNextOpenTime", "get_next_open_time"),
            ("AddWorkDuration", "add_work_duration"),
            ("TraceWorkDuration", "trace_work_duration"),
            ("GetWorkingDuration", "get_working_duration"),
            ("GetWorkingTicks", "get_working_ticks"),
            ("GetOpenWindows", "get_open_windows"),
            ("GetCapacityWindows", "get_capacity_windows"),
            ("GetUnavailableWindows", "get_unavailable_windows"),
            ("GetStateWindows", "get_state_windows"),
            ("GetWindowsAt", "get_windows_at"),
            ("GetCurrentOpenWindow", "get_current_open_window"),
            (
                "GetCurrentUnavailableWindow",
                "get_current_unavailable_window",
            ),
            ("GetNextTransitionTime", "get_next_transition_time"),
            ("GetPreviousTransitionTime", "get_previous_transition_time"),
            ("GetNearestTransitionTime", "get_nearest_transition_time"),
            ("FindFirstSlot", "find_first_slot"),
        ],
    )?;
    alias_attrs(
        m,
        "TimeGridStateWindow",
        &[
            ("Window", "window"),
            ("Capacity", "capacity"),
            ("CanWork", "can_work"),
        ],
    )?;
    alias_attrs(
        m,
        "TimeGridWindowMatch",
        &[
            ("Kind", "kind"),
            ("Window", "window"),
            ("Capacity", "capacity"),
        ],
    )?;
    alias_attrs(
        m,
        "TimeGridInstantAnalysis",
        &[
            ("Instant", "instant"),
            ("Capacity", "capacity"),
            ("CurrentWindow", "current_window"),
            ("PreviousTransition", "previous_transition"),
            ("NextTransition", "next_transition"),
            ("Matches", "matches"),
            ("CanWork", "can_work"),
        ],
    )?;
    alias_attrs(
        m,
        "TimeGridTimelineAnalysis",
        &[
            ("Window", "window"),
            ("Segments", "segments"),
            ("WorkingDuration", "working_duration"),
            ("WorkingTicks", "working_ticks"),
        ],
    )?;
    alias_attrs(
        m,
        "WorkingTimeTraceStep",
        &[("Window", "window"), ("Duration", "duration")],
    )?;
    alias_attrs(
        m,
        "WorkingTimeTrace",
        &[
            ("Start", "start"),
            ("RequestedDuration", "requested_duration"),
            ("Result", "result"),
            ("Steps", "steps"),
            ("RequestedTicks", "requested_ticks"),
            ("ConsumedTicks", "consumed_ticks"),
        ],
    )?;
    alias_attrs(
        m,
        "TimeGridPointQuery",
        &[
            ("CanWork", "can_work"),
            ("HasCapacity", "has_capacity"),
            ("GetCapacity", "get_capacity"),
            ("Analyze", "analyze"),
            ("GetWindows", "get_windows"),
            ("GetCurrentOpenWindow", "get_current_open_window"),
            (
                "GetCurrentUnavailableWindow",
                "get_current_unavailable_window",
            ),
            ("GetNextTransitionTime", "get_next_transition_time"),
            ("GetPreviousTransitionTime", "get_previous_transition_time"),
            ("GetNearestTransitionTime", "get_nearest_transition_time"),
            ("GetNextOpenTime", "get_next_open_time"),
            ("AddWorkDuration", "add_work_duration"),
            ("TraceWorkDuration", "trace_work_duration"),
        ],
    )?;
    alias_attrs(
        m,
        "TimeGridRangeQuery",
        &[
            ("GetOpenWindows", "get_open_windows"),
            ("GetUnavailableWindows", "get_unavailable_windows"),
            ("GetStateWindows", "get_state_windows"),
            ("Analyze", "analyze"),
            ("GetWorkingDuration", "get_working_duration"),
            ("GetWorkingTicks", "get_working_ticks"),
            ("GetCapacityWindows", "get_capacity_windows"),
            ("CanWork", "can_work"),
            ("HasCapacity", "has_capacity"),
            ("FindFirstSlot", "find_first_slot"),
        ],
    )?;
    alias_attrs(
        m,
        "TimeGridTimeline",
        &[
            ("Window", "window"),
            ("GetCapacityAt", "get_capacity_at"),
            ("GetCapacitiesAt", "get_capacities_at"),
            ("CanWork", "can_work"),
            ("Analyze", "analyze"),
            ("AnalyzeMany", "analyze_many"),
            ("AnalyzeRange", "analyze_range"),
            ("GetStateWindows", "get_state_windows"),
            ("GetOpenWindows", "get_open_windows"),
            ("GetUnavailableWindows", "get_unavailable_windows"),
            ("GetCapacityWindows", "get_capacity_windows"),
            ("GetWorkingDuration", "get_working_duration"),
            ("FindFirstSlot", "find_first_slot"),
        ],
    )?;
    alias_attrs(
        m,
        "TimeGridTimelineBatch",
        &[
            ("Create", "create"),
            ("Count", "count"),
            ("GetCapacitiesAt", "get_capacities_at"),
            ("Analyze", "analyze"),
        ],
    )?;
    alias_attrs(
        m,
        "TimeGridDefinition",
        &[
            ("ToCalendar", "to_calendar"),
            ("ToJson", "to_json"),
            ("FromJson", "from_json"),
        ],
    )?;
    Ok(())
}

fn alias_attrs(m: &Bound<'_, PyModule>, class_name: &str, pairs: &[(&str, &str)]) -> PyResult<()> {
    let class = m.getattr(class_name)?;
    for (source, target) in pairs {
        class.setattr(*target, class.getattr(*source)?)?;
    }
    Ok(())
}
