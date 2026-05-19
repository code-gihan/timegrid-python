# TimeGrid for Python

[![PyPI Version](https://img.shields.io/pypi/v/timegrid?logo=pypi&label=PyPI)](https://pypi.org/project/timegrid/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Runtime](https://img.shields.io/badge/core-Rust%20%2B%20PyO3-b7410e)

**Business-hours, availability, downtime, capacity, and SLA working-time calculations for Python, powered by Rust.**

TimeGrid turns schedule rules into a queryable operational timeline. Define working hours,
lunch breaks, holidays, maintenance windows, capacity overrides, and named schedule entries;
serialize the source definition as JSON; compile it once; then answer timestamp and range
queries quickly.

Use it for factory calendars, machine availability, SLA clocks, business-hour math,
capacity-aware slot search, and systems that need a dependable time layer before they
schedule, simulate, dispatch, or report.

> Not a scheduler. Not a simulator. Not a timezone database.
> TimeGrid is the availability and working-time engine those systems can depend on.

## Install

```bash
pip install timegrid
```

For supported platforms, `pip` automatically selects the matching prebuilt wheel from PyPI.

| Operating system | CPU architecture | Python | Wheel support |
| --- | --- | --- | --- |
| Windows | x86_64 / AMD64 | CPython 3.9+ | Prebuilt `win_amd64` wheel |
| Linux | x86_64 / AMD64 | CPython 3.9+ | Prebuilt `manylinux2014_x86_64` wheel |
| macOS | Intel x86_64 and Apple Silicon arm64 | CPython 3.9+ | Prebuilt `universal2` wheel |

The wheels use Python's stable ABI (`abi3`), so one wheel per OS/architecture supports
CPython 3.9 and newer. Other platforms or architectures can still install from the source
distribution when a Rust toolchain is available.

## 30-Second Example

Calculate an SLA due date while skipping lunch, nights, weekends, and one-off closures:

```python
from datetime import date, datetime, time
from timegrid import TimeGridCalendar, hours

calendar = (
    TimeGridCalendar.weekdays(time(9), time(17))
    .break_weekdays(time(12), time(13))
    .close(date(2026, 1, 1))
    .set_closed_window("company-offsite", datetime(2026, 1, 7, 13), datetime(2026, 1, 7, 17))
)

opened_at = datetime(2026, 1, 5, 11)
trace = calendar.at(opened_at).trace_work_duration(hours(10))

print(trace.result)
for step in trace.steps:
    print(step.window, step.duration)
```

## Practical Examples

Runnable examples live in [`examples/`](examples/):

```bash
python examples/sla_due_date.py
python examples/manufacturing_capacity_slot.py
python examples/fleet_batch_snapshot.py
python examples/json_schedule_template.py
python examples/overnight_shift.py
```

From a local checkout:

```bash
.\.venv\Scripts\python.exe examples\run_all.py
```

| Example | What it shows |
| --- | --- |
| `sla_due_date.py` | Business-hour due dates and consumed working windows. |
| `manufacturing_capacity_slot.py` | Downtime, capacity boosts, and first slot with enough capacity. |
| `fleet_batch_snapshot.py` | Batch capacity and analysis queries across many compiled machine timelines. |
| `json_schedule_template.py` | JSON schedule templates plus per-machine exceptions. |
| `overnight_shift.py` | Open rules that cross midnight and breaks inside overnight shifts. |

## What It Answers

| Question | API |
| --- | --- |
| Can this machine, team, or queue work now? | `timeline.analyze(now).can_work` |
| What is the effective capacity at this timestamp? | `analysis.capacity` |
| Which state window contains this timestamp? | `analysis.current_window` |
| When does the state change next? | `analysis.next_transition` |
| Why is this timestamp blocked or boosted? | `calendar.at(now).analyze().matches` |
| How much usable business time exists in a range? | `timeline.get_working_duration(start, end)` |
| Where is the first continuous slot with enough capacity? | `timeline.find_first_slot(start, hours(2), 3)` |
| What are many capacities at once? | `timeline.get_capacities_at(instants)` |
| What is the state of many machine timelines now? | `TimeGridTimelineBatch(timelines).get_capacities_at(now)` |

## Schedule Definitions

TimeGrid stores source rules, not compiled runtime state. That makes definitions easy to save,
review, ship, and apply to many operational entities:

```python
from datetime import datetime, time
from timegrid import TimeGridCalendar

template_json = (
    TimeGridCalendar.create()
    .open_weekdays(time(7), time(19))
    .break_weekdays(time(12), time(12, 45))
    .capacity(3)
    .to_json(indented=True)
)

template = TimeGridCalendar.from_json(template_json).to_definition()

machine = (
    template.to_calendar()
    .set_closed_window("machine-42-maintenance", datetime(2026, 1, 5, 15), datetime(2026, 1, 5, 17))
    .set_capacity_window("machine-42-overtime-crew", datetime(2026, 1, 5, 17), datetime(2026, 1, 5, 19), 5)
    .compile(datetime(2026, 1, 5), datetime(2026, 1, 6))
)
```

## Compile For Repeated Reads

Use `compile(start, end)` when the same calendar will be queried many times. A compiled
timeline is read-only and answers point and range queries by searching precomputed state
segments.

```python
from timegrid import TimeGridTimelineBatch, hours

timeline = calendar.compile(month_start, month_end)

point = timeline.analyze(now)
work = timeline.get_working_duration(day_start, day_end)
slot = timeline.find_first_slot(day_start, hours(4), 2)

capacities = timeline.get_capacities_at(instants)
analyses = timeline.analyze_many(instants)

fleet = TimeGridTimelineBatch(machine_timelines)
fleet_capacities = fleet.get_capacities_at(now)
```

## API Style

Python code can use Python naming conventions while compatibility aliases preserve the
original TimeGrid.NET-style names.

| Surface | Python style | Compatibility style |
| --- | --- | --- |
| Classes | `TimeGridCalendar`, `TimeWindow` | same |
| Methods and properties | `snake_case` | `PascalCase` |
| Constants | `DayOfWeek.MONDAY` | `DayOfWeek.Monday` |

## Why It Is Fast

| Operation | Runtime strategy |
| --- | --- |
| Point analysis | Binary search over compiled state segments |
| Range analysis | Binary search, then scan touched segments only |
| Transition lookup | Segment boundary lookup |
| Working duration | Sum usable segment durations |
| Slot search | Scan continuous capacity-matching segments |
| Batch point queries | One Python call, Rust loop over compiled timelines |
| JSON | Rust `serde` source definition serialization |

Local quick-query benchmark on CPython 3.12, Windows 10, Rust release wheel.
The scenario matches the TimeGrid.NET `QuickQueryPerf` benchmark: 50,000 compiled state
segments, 1,000 compiled machine timelines, 20,000 warmup calls, and 1,000,000 measured calls.

Common API, one Python call per query:

| Scenario | Operation | Python/Rust | TimeGrid.NET |
| --- | --- | ---: | ---: |
| 50,000 compiled state segments | `get_capacity_at` | 0.153 us/query | 0.052 us/query |
| 50,000 compiled state segments | `analyze` | 0.225 us/query | 0.064 us/query |
| 1,000 compiled machine timelines | `get_capacity_at` sweep | 123.154 us | 26.625 us |
| 1,000 compiled machine timelines | `analyze` sweep | 189.991 us | 34.463 us |

Batch API, one Python call per batch:

| Scenario | Operation | Python/Rust |
| --- | --- | ---: |
| 50,000 compiled state segments | `get_capacities_at` | 0.076 us/query |
| 50,000 compiled state segments | `analyze_many` | 0.220 us/query |
| 1,000 compiled machine timelines | `TimeGridTimelineBatch.get_capacities_at` sweep | 28.342 us |
| 1,000 compiled machine timelines | `TimeGridTimelineBatch.analyze` sweep | 118.398 us |

Measurements exclude compile time and run against already compiled timelines.

## Good Fit

| Use TimeGrid for | Use something else for |
| --- | --- |
| Business-hours and working-time calculations | Calendar UI rendering |
| Manufacturing availability and machine downtime | Queue dispatching |
| Capacity-aware slot search | Optimization solving |
| SLA clocks and operational reporting | Full timezone database behavior |
| JSON-backed schedule definitions | General-purpose date-range value objects |
| Scheduler or simulator availability layer | Running background jobs |

## Core Rules

- Time windows are half-open: `[start, end)`.
- Capacity `0` means unavailable.
- Capacity overrides split the timeline into state segments.
- JSON stores source definitions, not compiled timelines.
- Compiled timelines are read-only and optimized for repeated analysis.
- Use naive `datetime` values consistently in your chosen local or UTC convention.

## Direct APIs

```python
calendar.can_work(now)
calendar.get_capacity_at(now)
calendar.get_windows_at(now)
calendar.get_previous_transition_time(now)
calendar.get_next_transition_time(now)
calendar.get_open_windows(start, end)
calendar.get_unavailable_windows(start, end)
calendar.get_state_windows(start, end)
calendar.get_working_duration(start, end)
calendar.find_first_slot(start, hours(2), 2)

timeline.get_capacities_at(instants)
timeline.analyze_many(instants)
TimeGridTimelineBatch(timelines).get_capacities_at(now)
```

## Links

- PyPI: https://pypi.org/project/timegrid/
- Repository: https://github.com/code-gihan/timegrid-python
- Issues: https://github.com/code-gihan/timegrid-python/issues
- TimeGrid.NET: https://www.nuget.org/packages/TimeGrid.NET
- License: MIT

If TimeGrid gives you a wrong answer, a confusing error, or a missing API for a real scheduling
or availability problem, please open a GitHub issue with the smallest calendar definition and
query that reproduces it.
