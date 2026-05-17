from datetime import datetime, time

from timegrid import DayOfWeek, TimeGridCalendar, hours


def fmt(value):
    return value.strftime("%Y-%m-%d %H:%M")


def main():
    calendar = (
        TimeGridCalendar.create()
        .add_open_rule(DayOfWeek.MONDAY, time(22), time(2))
        .add_break_rule(DayOfWeek.TUESDAY, time(0, 30), time(0, 45))
        .capacity(1)
    )

    start = datetime(2026, 1, 5, 22)
    end = datetime(2026, 1, 6, 3)
    timeline = calendar.compile(start, end)
    slot = timeline.find_first_slot(start, hours(2))

    print("Overnight shift")
    print(f"22:30 capacity: {timeline.get_capacity_at(datetime(2026, 1, 5, 22, 30))}")
    print(f"00:35 capacity: {timeline.get_capacity_at(datetime(2026, 1, 6, 0, 35))}")
    print(f"01:00 capacity: {timeline.get_capacity_at(datetime(2026, 1, 6, 1))}")
    print(f"Working duration from {fmt(start)} to {fmt(end)}: {timeline.get_working_duration(start, end)}")
    print(f"First 2-hour slot: {fmt(slot.start)} -> {fmt(slot.end) if slot else 'not found'}")


if __name__ == "__main__":
    main()
