from datetime import date, datetime, time, timedelta

import pytest

from timegrid import (
    DayOfWeek,
    Hours,
    Minutes,
    TimeGridCalendar,
    TimeGridEntryKind,
    TimeGridWindowKind,
    TimeWindow,
)


def day(number: int, hour: int = 0, minute: int = 0) -> datetime:
    return datetime(2026, 1, number, hour, minute)


def workday() -> TimeGridCalendar:
    return TimeGridCalendar.Weekdays(time(9), time(18)).BreakWeekdays(time(12), time(13))


def test_weekday_calendar_excludes_lunch():
    calendar = workday()

    assert calendar.CanWork(day(5, 11, 30))
    assert not calendar.CanWork(day(5, 12, 30))
    assert calendar.CanWork(day(5, 13, 30))


def test_working_duration_excludes_lunch_and_holiday():
    calendar = workday().Close(date(2026, 1, 6))

    assert calendar.GetWorkingDuration(day(5, 9), day(7, 18)) == Hours(16)
    assert calendar.GetWorkingTicks(day(5, 9), day(5, 18)) == 8 * 60 * 60 * 10_000_000


def test_add_work_duration_skips_unavailable_time():
    calendar = workday()

    assert calendar.AddWorkDuration(day(5, 11), Hours(3)) == day(5, 15)
    trace = calendar.TraceWorkDuration(day(5, 11), Hours(3))

    assert trace.Result == day(5, 15)
    assert trace.ConsumedTicks == 3 * 60 * 60 * 10_000_000
    assert [step.Window for step in trace.Steps] == [
        TimeWindow(day(5, 11), day(5, 12)),
        TimeWindow(day(5, 13), day(5, 15)),
    ]


def test_capacity_and_first_slot():
    calendar = (
        TimeGridCalendar.Create()
        .OpenWeekdays(time(9), time(18))
        .Capacity(2)
        .Capacity(day(5, 10), day(5, 12), 5)
        .Close(date(2026, 1, 10))
    )

    assert calendar.GetCapacityAt(day(5, 9, 30)) == 2
    assert calendar.GetCapacityAt(day(5, 10, 30)) == 5
    assert calendar.GetCapacityAt(day(10, 10, 30)) == 0
    assert calendar.FindFirstSlot(day(5, 9), Hours(2), minimumCapacity=5) == TimeWindow(
        day(5, 10), day(5, 12)
    )


def test_named_entries_are_crud_style_windows():
    calendar = (
        workday()
        .SetClosedWindow("maintenance", day(5, 14), day(5, 15))
        .SetCapacityWindow("boost", day(5, 10), day(5, 11), 4)
    )

    assert calendar.GetEntry("maintenance").Kind == TimeGridEntryKind.Closed
    assert calendar.GetCapacityAt(day(5, 10, 30)) == 4
    assert not calendar.CanWork(day(5, 14, 30))
    assert calendar.RemoveEntry("maintenance")
    assert not calendar.RemoveEntry("maintenance")
    assert calendar.CanWork(day(5, 14, 30))


def test_analysis_and_matches():
    calendar = workday().Capacity(day(5, 10), day(5, 11), 3)
    analysis = calendar.Analyze(day(5, 10, 30))

    assert analysis.CanWork
    assert analysis.Capacity == 3
    assert analysis.CurrentWindow == TimeWindow(day(5, 10), day(5, 11))
    assert any(
        match.Kind == TimeGridWindowKind.CapacityOverride and match.Capacity == 3
        for match in analysis.Matches
    )


def test_compiled_timeline_matches_calendar():
    calendar = workday().Capacity(day(5, 10), day(5, 11), 3)
    timeline = calendar.Compile(day(5, 8), day(5, 18))

    assert timeline.GetStateWindows(day(5, 8), day(5, 18)) == calendar.GetStateWindows(
        day(5, 8), day(5, 18)
    )
    assert timeline.GetWorkingDuration(day(5, 8), day(5, 18)) == Hours(8)
    assert timeline.FindFirstSlot(day(5, 8), Hours(2)) == TimeWindow(day(5, 9), day(5, 11))
    assert timeline.Analyze(day(5, 12, 30)).CurrentWindow == TimeWindow(day(5, 12), day(5, 13))
    assert timeline.Analyze(day(5, 8), day(5, 18)).WorkingDuration == Hours(8)


def test_json_roundtrip_uses_source_definition():
    calendar = (
        workday()
        .SetClosedWindow("planned-maintenance", day(5, 23), day(6, 1))
        .SetCapacityWindow("line-boost", day(5, 10), day(5, 11), 5)
    )

    json = calendar.ToJson(indented=True)
    restored = TimeGridCalendar.FromJson(json)

    assert "openRules" in json
    assert restored.GetStateWindows(day(5, 8), day(6, 3)) == calendar.GetStateWindows(
        day(5, 8), day(6, 3)
    )
    assert restored.GetCapacityAt(day(5, 10, 30)) == 5
    assert restored.GetEntry("planned-maintenance").Kind == TimeGridEntryKind.Closed


def test_range_and_point_queries():
    calendar = workday()
    start = day(5, 11)

    assert calendar.At(start).AddWorkDuration(Hours(3)) == day(5, 15)
    assert calendar.Between(day(5, 9), day(5, 18)).GetWorkingDuration() == Hours(8)
    assert calendar.Between(day(5, 9), day(5, 12)).CanWork()
    assert not calendar.Between(day(5, 11), day(5, 14)).CanWork()


def test_intersection_uses_minimum_capacity():
    factory = TimeGridCalendar.Create().OpenWeekdays(time(9), time(18)).Capacity(5)
    machine = TimeGridCalendar.Create().Open(day(5, 10), day(5, 16)).Capacity(
        day(5, 13), day(5, 16), 2
    )
    calendar = factory.And(machine)

    assert calendar.IsComposite
    assert calendar.GetCapacityAt(day(5, 9, 30)) == 0
    assert calendar.GetCapacityAt(day(5, 10, 30)) == 1
    assert calendar.GetCapacityAt(day(5, 14, 30)) == 2


def test_window_helpers_and_validation():
    window = TimeWindow(day(5, 9), day(5, 11))

    assert window.Contains(day(5, 10))
    assert not window.Contains(day(5, 11))
    assert window.Overlaps(TimeWindow(day(5, 10), day(5, 12)))
    assert window.Intersect(TimeWindow(day(5, 10), day(5, 12))) == TimeWindow(
        day(5, 10), day(5, 11)
    )

    with pytest.raises(ValueError):
        TimeGridCalendar.Create().AddOpenRule(DayOfWeek.Monday, time(9), time(9))
    with pytest.raises(ValueError):
        calendar = TimeGridCalendar.Create()
        calendar.GetWorkingDuration(day(5, 10), day(5, 9))
    with pytest.raises(ValueError):
        TimeGridCalendar.Create().FindFirstSlot(day(5, 9), Hours(-1))


def test_cross_midnight_rules():
    calendar = TimeGridCalendar.Create().AddOpenRule(DayOfWeek.Monday, time(22), time(2))

    assert calendar.CanWork(day(5, 23))
    assert calendar.CanWork(day(6, 1))
    assert not calendar.CanWork(day(6, 3))


def test_duration_helpers():
    assert Hours(8) == timedelta(hours=8)
    assert Minutes(30) == timedelta(minutes=30)
