from datetime import datetime, time

from timegrid import TimeGridCalendar, TimeGridEntryKind


ENTRY_KIND_NAMES = {
    TimeGridEntryKind.OPEN: "open",
    TimeGridEntryKind.CLOSED: "closed",
    TimeGridEntryKind.CAPACITY: "capacity",
}


def main():
    factory_template = (
        TimeGridCalendar.create()
        .open_weekdays(time(7), time(19))
        .break_weekdays(time(12), time(12, 45))
        .capacity(3)
        .to_json(indented=True)
    )

    restored_template = TimeGridCalendar.from_json(factory_template).to_definition()
    machine_calendar = (
        restored_template.to_calendar()
        .set_closed_window("machine-42-maintenance", datetime(2026, 1, 5, 15), datetime(2026, 1, 5, 17))
        .set_capacity_window("machine-42-overtime-crew", datetime(2026, 1, 5, 17), datetime(2026, 1, 5, 19), 5)
    )
    timeline = machine_calendar.compile(datetime(2026, 1, 5), datetime(2026, 1, 6))

    print("JSON schedule template")
    print(f"Template JSON length: {len(factory_template)} characters")
    print(f"15:30 capacity: {timeline.get_capacity_at(datetime(2026, 1, 5, 15, 30))}")
    print(f"17:30 capacity: {timeline.get_capacity_at(datetime(2026, 1, 5, 17, 30))}")
    print("Named entries:")

    for entry in machine_calendar.entries:
        print(f"  - {entry.id}: kind={ENTRY_KIND_NAMES[entry.kind]}, capacity={entry.capacity}")


if __name__ == "__main__":
    main()
