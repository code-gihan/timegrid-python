from datetime import date, datetime, time

from timegrid import TimeGridCalendar, hours


def fmt(value):
    return value.strftime("%Y-%m-%d %H:%M")


def fmt_window(window):
    if window is None:
        return "not found"
    return f"{fmt(window.start)} -> {fmt(window.end)}"


def main():
    calendar = (
        TimeGridCalendar.create()
        .open_weekdays(time(6), time(22))
        .break_weekdays(time(12), time(12, 30))
        .capacity(2)
        .close(date(2026, 1, 1))
        .set_closed_window("press-17-maintenance", datetime(2026, 1, 5, 9, 30), datetime(2026, 1, 5, 11))
        .set_capacity_window("press-17-extra-operator", datetime(2026, 1, 5, 13), datetime(2026, 1, 5, 16), 4)
    )

    timeline = calendar.compile(datetime(2026, 1, 5), datetime(2026, 1, 6))
    blocked = timeline.analyze(datetime(2026, 1, 5, 10))
    boosted = timeline.analyze(datetime(2026, 1, 5, 14))
    slot = timeline.find_first_slot(datetime(2026, 1, 5, 6), hours(2), 4, datetime(2026, 1, 5, 18))

    print("Manufacturing capacity slot")
    print(f"10:00 can work: {blocked.can_work}, capacity: {blocked.capacity}")
    print(f"14:00 can work: {boosted.can_work}, capacity: {boosted.capacity}")
    print(f"First 2-hour slot with capacity >= 4: {fmt_window(slot)}")
    print("State windows from 09:00 to 16:00:")

    for state in timeline.get_state_windows(datetime(2026, 1, 5, 9), datetime(2026, 1, 5, 16)):
        print(f"  - {fmt_window(state.window)} capacity={state.capacity}")


if __name__ == "__main__":
    main()
