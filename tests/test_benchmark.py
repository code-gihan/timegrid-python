from datetime import datetime, timedelta

from timegrid import Minutes, TimeGridCalendar


def test_compiled_timeline_capacity_benchmark(benchmark):
    start = datetime(2026, 1, 1)
    end = start + timedelta(minutes=10_000)
    calendar = TimeGridCalendar.Create().Open(start, end)

    for index in range(10_000):
        minute = start + timedelta(minutes=index)
        calendar.Capacity(minute, minute + timedelta(minutes=1), (index % 7) + 1)

    timeline = calendar.Compile(start, end)
    probe = start + timedelta(minutes=7_777)

    capacity = benchmark(lambda: timeline.GetCapacityAt(probe))

    assert capacity == ((7_777 % 7) + 1)
    assert timeline.FindFirstSlot(start, Minutes(1), minimumCapacity=7) is not None
