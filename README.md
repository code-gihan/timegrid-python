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

## What It Answers

| Question | API |
| --- | --- |
| Can this machine work now? | `timeline.Analyze(now).CanWork` |
| Which state window contains this timestamp? | `analysis.CurrentWindow` |
| What is the effective capacity? | `analysis.Capacity` |
| When does the state change? | `analysis.NextTransition` |
| Why is this timestamp blocked? | `calendar.At(now).Analyze().Matches` |
| How much usable time exists in a range? | `timeline.GetWorkingDuration(start, end)` |
| Where is the first slot with enough capacity? | `timeline.FindFirstSlot(start, Hours(2), 3)` |

## Quick Start

```python
from datetime import date, datetime, time
from timegrid import Hours, TimeGridCalendar

month_start = datetime(2026, 1, 1)
month_end = datetime(2026, 2, 1)

timeline = (
    TimeGridCalendar
    .Weekdays(time(9), time(18))
    .BreakWeekdays(time(12), time(13))
    .Close(date(2026, 1, 1))
    .Down(datetime(2026, 1, 5, 15), datetime(2026, 1, 5, 16))
    .Capacity(datetime(2026, 1, 5, 9), datetime(2026, 1, 5, 12), 3)
    .Compile(month_start, month_end)
)

state = timeline.Analyze(datetime(2026, 1, 5, 10, 30))

print(state.CanWork)
print(state.Capacity)
print(state.CurrentWindow)
print(state.NextTransition)
```

## JSON Definitions

Store source schedules, not compiled runtime state.

```python
from datetime import datetime, time
from timegrid import TimeGridCalendar

calendar = (
    TimeGridCalendar
    .Weekdays(time(8), time(20))
    .BreakWeekdays(time(12), time(13))
    .SetClosedWindow("machine-17-maintenance", down_start, down_end)
    .SetCapacityWindow("machine-17-boost", boost_start, boost_end, 4)
)

json = calendar.ToJson(indented=True)

restored = (
    TimeGridCalendar
    .FromJson(json)
    .Compile(month_start, month_end)
)
```

## Manufacturing Pattern

Use one shared template and add equipment-specific exceptions.

```python
template = (
    TimeGridCalendar
    .Weekdays(time(9), time(18))
    .BreakWeekdays(time(12), time(13))
    .ToDefinition()
)

machine = (
    template
    .ToCalendar()
    .SetClosedWindow("maintenance", maintenance_start, maintenance_end)
    .SetCapacityWindow("extra-line", boost_start, boost_end, 3)
    .Compile(month_start, month_end)
)

state = machine.Analyze(now)
```

This keeps common schedule rules reusable while each machine carries only its own downtime and capacity changes.

## Why It Is Fast

Use `Compile(start, end)` for read-heavy systems:

```python
timeline = calendar.Compile(month_start, month_end)

point = timeline.Analyze(now)
work = timeline.GetWorkingDuration(day_start, day_end)
slot = timeline.FindFirstSlot(day_start, Hours(4), minimumCapacity=2)
```

| Operation | Runtime strategy |
| --- | --- |
| Point analysis | Binary search over compiled state segments |
| Range analysis | Binary search, then scan touched segments only |
| Transition lookup | Segment boundary lookup |
| Working duration | Sum usable segment durations |
| Slot search | Scan continuous capacity-matching segments |
| JSON | Rust `serde` source definition serialization |

Local benchmark on CPython 3.12, Windows 10, Rust release build:

| Scenario | Operation | Result |
| --- | --- | ---: |
| 10,000 compiled state segments | `GetCapacityAt` | 0.259 us/query |

Measurements exclude compile time and run against an already compiled timeline.

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
analysis = timeline.Analyze(now)

if analysis.CanWork:
    print(f"capacity: {analysis.Capacity}")
```

## Named Timeline Entries

```python
calendar = (
    TimeGridCalendar
    .Create()
    .SetOpenWindow("shift-a", start, end)
    .SetClosedWindow("maintenance", down_start, down_end)
    .SetCapacityWindow("line-boost", boost_start, boost_end, 4)
)

maintenance = calendar.GetEntry("maintenance")

calendar.SetClosedWindow("maintenance", new_down_start, new_down_end)
calendar.RemoveEntry("maintenance")
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
calendar.CanWork(now)
calendar.GetCapacityAt(now)
calendar.GetWindowsAt(now)
calendar.GetPreviousTransitionTime(now)
calendar.GetNextTransitionTime(now)
calendar.GetOpenWindows(start, end)
calendar.GetUnavailableWindows(start, end)
calendar.GetStateWindows(start, end)
calendar.GetWorkingDuration(start, end)
calendar.FindFirstSlot(start, Hours(2), minimumCapacity=2)
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
