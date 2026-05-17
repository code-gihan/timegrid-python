from datetime import date, datetime, time

from timegrid import TimeGridCalendar, hours


def fmt(value):
    return value.strftime("%Y-%m-%d %H:%M")


def main():
    calendar = (
        TimeGridCalendar.weekdays(time(9), time(17))
        .break_weekdays(time(12), time(13))
        .close(date(2026, 1, 1))
        .set_closed_window("company-offsite", datetime(2026, 1, 7, 13), datetime(2026, 1, 7, 17))
    )

    opened_at = datetime(2026, 1, 5, 11)
    response_time = hours(10)
    trace = calendar.at(opened_at).trace_work_duration(response_time)

    print("SLA due date")
    print(f"Ticket opened: {fmt(opened_at)}")
    print(f"Working time requested: {response_time}")
    print(f"Due at: {fmt(trace.result)}")
    print("Consumed windows:")

    for step in trace.steps:
        print(f"  - {fmt(step.window.start)} -> {fmt(step.window.end)} ({step.duration})")

    same_range = calendar.between(opened_at, trace.result).get_working_duration()
    print(f"Verified working time in range: {same_range}")


if __name__ == "__main__":
    main()
