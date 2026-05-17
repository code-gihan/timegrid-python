from datetime import datetime, time

from timegrid import TimeGridCalendar, TimeGridTimelineBatch


def build_machine(name, maintenance_start, maintenance_end, boost_start, boost_end, boost_capacity):
    calendar = (
        TimeGridCalendar.create()
        .open_weekdays(time(8), time(20))
        .break_weekdays(time(12), time(13))
        .capacity(2)
        .set_closed_window(f"{name}-maintenance", maintenance_start, maintenance_end)
        .set_capacity_window(f"{name}-boost", boost_start, boost_end, boost_capacity)
    )

    timeline = calendar.compile(datetime(2026, 1, 5), datetime(2026, 1, 6))
    return name, timeline


def main():
    machines = [
        build_machine(
            "press-17",
            datetime(2026, 1, 5, 9),
            datetime(2026, 1, 5, 10),
            datetime(2026, 1, 5, 13),
            datetime(2026, 1, 5, 16),
            4,
        ),
        build_machine(
            "lathe-03",
            datetime(2026, 1, 5, 14),
            datetime(2026, 1, 5, 15),
            datetime(2026, 1, 5, 10),
            datetime(2026, 1, 5, 12),
            3,
        ),
        build_machine(
            "paint-02",
            datetime(2026, 1, 5, 16),
            datetime(2026, 1, 5, 18),
            datetime(2026, 1, 5, 13),
            datetime(2026, 1, 5, 17),
            5,
        ),
    ]

    names = [name for name, _ in machines]
    timelines = [timeline for _, timeline in machines]
    batch = TimeGridTimelineBatch.create(timelines)

    instant = datetime(2026, 1, 5, 14, 30)
    capacities = batch.get_capacities_at(instant)
    analyses = batch.analyze(instant)

    print("Fleet batch snapshot")
    print(f"Machines queried: {batch.count}")
    print(f"Instant: {instant:%Y-%m-%d %H:%M}")

    for name, capacity, analysis in zip(names, capacities, analyses):
        state = "available" if analysis.can_work else "blocked"
        print(f"  - {name}: {state}, capacity={capacity}")

    capable = [name for name, capacity in zip(names, capacities) if capacity >= 4]
    print(f"Machines with capacity >= 4: {', '.join(capable) if capable else 'none'}")


if __name__ == "__main__":
    main()
