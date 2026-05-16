# TimeGrid

[![PyPI Version](https://img.shields.io/pypi/v/timegrid?logo=pypi&label=PyPI)](https://pypi.org/project/timegrid/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Runtime](https://img.shields.io/badge/core-Rust%20%2B%20PyO3-b7410e)

**TimeGrid is a lightweight compiled timeline engine for Python, powered by Rust.**

Define availability, breaks, downtime, holidays, capacity, and named schedule entries.
Serialize the source definition as JSON. Compile it once. Analyze timestamps and ranges fast.

> Not a scheduler. Not a simulator. Not a DateRange wrapper.
> TimeGrid is the operational time layer those systems can depend on.

## Install

```bash
pip install timegrid
```

## API Style

TimeGrid follows Python naming conventions while keeping the original TimeGrid.NET-style names as compatibility aliases.

| Surface | Style | Example |
| --- | --- | --- |
| Classes | `PascalCase` | `TimeGridCalendar`, `TimeWindow` |
| Methods and properties | `snake_case` | `set_closed_window`, `current_window` |
| Constants | `UPPER_CASE` | `DayOfWeek.MONDAY` |

## What It Answers

| Question | API |
| --- | --- |
| Can this machine work now? | `timeline.analyze(now).can_work` |
| Which state window contains this timestamp? | `analysis.current_window` |
| What is the effective capacity? | `analysis.capacity` |
| When does the state change? | `analysis.next_transition` |
| Why is this timestamp blocked? | `calendar.at(now).analyze().matches` |
| How much usable time exists in a range? | `timeline.get_working_duration(start, end)` |
| Where is the first slot with enough capacity? | `timeline.find_first_slot(start, hours(2), 3)` |
| What are many capacities at once? | `timeline.get_capacities_at(instants)` |
| What is the state of many machines now? | `TimeGridTimelineBatch(machines).get_capacities_at(now)` |

## Quick Start

```python
from datetime import date, datetime, time
from timegrid import TimeGridCalendar

month_start = datetime(2026, 1, 1)
month_end = datetime(2026, 2, 1)

timeline = (
    TimeGridCalendar
    .weekdays(time(9), time(18))
    .break_weekdays(time(12), time(13))
    .close(date(2026, 1, 1))
    .down(datetime(2026, 1, 5, 15), datetime(2026, 1, 5, 16))
    .capacity(datetime(2026, 1, 5, 9), datetime(2026, 1, 5, 12), 3)
    .compile(month_start, month_end)
)

state = timeline.analyze(datetime(2026, 1, 5, 10, 30))

print(state.can_work)
print(state.capacity)
print(state.current_window)
print(state.next_transition)
```

## JSON Definitions

Store source schedules, not compiled runtime state.

```python
from datetime import datetime, time
from timegrid import TimeGridCalendar

calendar = (
    TimeGridCalendar
    .weekdays(time(8), time(20))
    .break_weekdays(time(12), time(13))
    .set_closed_window("machine-17-maintenance", down_start, down_end)
    .set_capacity_window("machine-17-boost", boost_start, boost_end, 4)
)

json = calendar.to_json(indented=True)

restored = (
    TimeGridCalendar
    .from_json(json)
    .compile(month_start, month_end)
)
```

## Manufacturing Pattern

Use one shared template and add equipment-specific exceptions.

```python
template = (
    TimeGridCalendar
    .weekdays(time(9), time(18))
    .break_weekdays(time(12), time(13))
    .to_definition()
)

machine = (
    template
    .to_calendar()
    .set_closed_window("maintenance", maintenance_start, maintenance_end)
    .set_capacity_window("extra-line", boost_start, boost_end, 3)
    .compile(month_start, month_end)
)

state = machine.analyze(now)
```

This keeps common schedule rules reusable while each machine carries only its own downtime and capacity changes.

## Why It Is Fast

Use `compile(start, end)` for read-heavy systems:

```python
from timegrid import hours

timeline = calendar.compile(month_start, month_end)

point = timeline.analyze(now)
work = timeline.get_working_duration(day_start, day_end)
slot = timeline.find_first_slot(day_start, hours(4), 2)
```

For repeated reads, batch the natural unit of work:

```python
capacities = timeline.get_capacities_at(instants)
analyses = timeline.analyze_many(instants)

fleet = TimeGridTimelineBatch(machine_timelines)
fleet_capacities = fleet.get_capacities_at(now)
fleet_analyses = fleet.analyze(now)
```

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
The common scenario matches the TimeGrid.NET `QuickQueryPerf` benchmark: 50,000 compiled state segments, 1,000 compiled machine timelines, 20,000 warmup calls, and 1,000,000 measured calls.

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

Measurements exclude compile time and run against already compiled timelines. Common API checksums matched TimeGrid.NET at `14277834`; the combined Common + Batch verification checksum was `28681706`. Batch capacity queries remove most Python method-dispatch overhead, while analysis still pays for creating Python result objects.

## Before / After

Without TimeGrid, operational time rules spread across conditionals:

```python
if instant.weekday() >= 5:
    return False
if time(9) > instant.time() or instant.time() >= time(18):
    return False
if time(12) <= instant.time() < time(13):
    return False
if instant.date() in holidays:
    return False
if downtime_start <= instant < downtime_end:
    return False

return capacity > 0
```

With TimeGrid, rules are data and queries stay small:

```python
analysis = timeline.analyze(now)

if analysis.can_work:
    print(f"capacity: {analysis.capacity}")
```

## Named Timeline Entries

```python
calendar = (
    TimeGridCalendar
    .create()
    .set_open_window("shift-a", start, end)
    .set_closed_window("maintenance", down_start, down_end)
    .set_capacity_window("line-boost", boost_start, boost_end, 4)
)

maintenance = calendar.get_entry("maintenance")

calendar.set_closed_window("maintenance", new_down_start, new_down_end)
calendar.remove_entry("maintenance")
```

## Good Fit

| Use TimeGrid for | Use something else for |
| --- | --- |
| Manufacturing availability | Running background jobs |
| Machine downtime analysis | Queue dispatching |
| Capacity-aware slot search | Calendar UI rendering |
| SLA working-time math | Full time-zone database behavior |
| Scheduler/simulator time layer | Optimization solving |
| JSON-backed schedule definitions | General-purpose DateRange value objects |

## Core Rules

- Time windows are half-open: `[start, end)`.
- Capacity `0` means unavailable.
- Capacity overrides split the timeline into state segments.
- JSON stores source definitions, not compiled timelines.
- Compiled timelines are read-only and optimized for repeated analysis.
- Use naive `datetime` values consistently in your chosen local or UTC convention.

<details>
<summary>Direct APIs</summary>

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
TimeGridTimelineBatch(timelines).analyze(now)
```

</details>

## Development

```bash
python -m venv .venv
.venv\Scripts\python -m pip install maturin pytest pytest-benchmark
.venv\Scripts\maturin develop --manifest-path .\timegrid\Cargo.toml --release
.venv\Scripts\python -m pytest -q
.venv\Scripts\maturin build --manifest-path .\timegrid\Cargo.toml --release
```

## Links

- PyPI: https://pypi.org/project/timegrid/
- Repository: https://github.com/code-gihan/timegrid-python
- License: MIT
