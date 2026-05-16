from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, time, timedelta
from time import perf_counter

from timegrid import TimeGridCalendar, TimeGridTimelineBatch


QUERY_COUNT = 1_000_000
WARMUP_COUNT = 20_000


@dataclass(frozen=True)
class Metric:
    total_ms: float
    avg_us: float


@dataclass(frozen=True)
class QuickQueryResult:
    get_capacity_50k: Metric
    analyze_50k: Metric
    machines_get_capacity: Metric
    machines_analyze: Metric
    batch_get_capacity_50k: Metric
    batch_analyze_50k: Metric
    batch_machines_get_capacity: Metric
    batch_machines_analyze: Metric
    checksum: int


def day(day_number: int, hour: int, minute: int = 0) -> datetime:
    return datetime(2026, 1, day_number, hour, minute)


def measure(action, query_count: int = QUERY_COUNT) -> tuple[Metric, int]:
    started = perf_counter()
    checksum = action()
    elapsed = perf_counter() - started
    return Metric(total_ms=elapsed * 1_000, avg_us=elapsed * 1_000_000 / query_count), checksum


def run_quick_query_perf() -> QuickQueryResult:
    checksum = 0

    start = day(5, 0)
    calendar = TimeGridCalendar.Create().Open(start, start + timedelta(minutes=50_000))

    for index in range(50_000):
        minute = start + timedelta(minutes=index)
        calendar.Capacity(minute, minute + timedelta(minutes=1), (index % 7) + 1)

    timeline = calendar.Compile(start, start + timedelta(minutes=50_000))
    instants = [start + timedelta(minutes=index, seconds=1) for index in range(50_000)]

    def run_get_capacity(count: int) -> int:
        total = 0
        length = len(instants)
        get_capacity = timeline.GetCapacityAt
        for index in range(count):
            total += get_capacity(instants[index % length])
        return total

    def run_analyze(count: int) -> int:
        total = 0
        length = len(instants)
        analyze = timeline.Analyze
        for index in range(count):
            total += analyze(instants[index % length]).Capacity
        return total

    checksum += run_get_capacity(WARMUP_COUNT)
    get_capacity_50k, value = measure(lambda: run_get_capacity(QUERY_COUNT))
    checksum += value

    checksum += run_analyze(WARMUP_COUNT)
    analyze_50k, value = measure(lambda: run_analyze(QUERY_COUNT))
    checksum += value

    batch_rounds_50k = QUERY_COUNT // len(instants)

    def run_batch_get_capacity(rounds: int) -> int:
        total = 0
        get_capacities = timeline.GetCapacitiesAt
        for _ in range(rounds):
            total += sum(get_capacities(instants))
        return total

    def run_batch_analyze(rounds: int) -> int:
        total = 0
        analyze_many = timeline.AnalyzeMany
        for _ in range(rounds):
            total += sum(analysis.Capacity for analysis in analyze_many(instants))
        return total

    checksum += run_batch_get_capacity(1)
    batch_get_capacity_50k, value = measure(
        lambda: run_batch_get_capacity(batch_rounds_50k),
        query_count=batch_rounds_50k * len(instants),
    )
    checksum += value

    checksum += run_batch_analyze(1)
    batch_analyze_50k, value = measure(
        lambda: run_batch_analyze(batch_rounds_50k),
        query_count=batch_rounds_50k * len(instants),
    )
    checksum += value

    template = (
        TimeGridCalendar.Weekdays(time(9), time(18))
        .BreakWeekdays(time(12), time(13))
        .ToDefinition()
    )
    horizon_start = day(5, 0)
    horizon_end = day(12, 0)
    machines = [
        template.ToCalendar()
        .SetClosedWindow(f"machine-{index}-maintenance", day(5, 10), day(5, 11))
        .SetCapacityWindow(f"machine-{index}-capacity", day(5, 13), day(5, 15), 2 + (index % 3))
        .Compile(horizon_start, horizon_end)
        for index in range(1_000)
    ]
    probe = day(5, 13, 30)
    machine_batch = TimeGridTimelineBatch(machines)

    def run_machines_get_capacity(count: int) -> int:
        total = 0
        length = len(machines)
        for index in range(count):
            total += machines[index % length].GetCapacityAt(probe)
        return total

    def run_machines_analyze(count: int) -> int:
        total = 0
        length = len(machines)
        for index in range(count):
            total += machines[index % length].Analyze(probe).Capacity
        return total

    checksum += run_machines_get_capacity(WARMUP_COUNT)
    machines_get_capacity, value = measure(lambda: run_machines_get_capacity(QUERY_COUNT))
    checksum += value

    checksum += run_machines_analyze(WARMUP_COUNT)
    machines_analyze, value = measure(lambda: run_machines_analyze(QUERY_COUNT))
    checksum += value

    batch_machine_rounds = QUERY_COUNT // len(machines)

    def run_batch_machines_get_capacity(rounds: int) -> int:
        total = 0
        get_capacities = machine_batch.GetCapacitiesAt
        for _ in range(rounds):
            total += sum(get_capacities(probe))
        return total

    def run_batch_machines_analyze(rounds: int) -> int:
        total = 0
        analyze = machine_batch.Analyze
        for _ in range(rounds):
            total += sum(analysis.Capacity for analysis in analyze(probe))
        return total

    checksum += run_batch_machines_get_capacity(1)
    batch_machines_get_capacity, value = measure(
        lambda: run_batch_machines_get_capacity(batch_machine_rounds),
        query_count=batch_machine_rounds * len(machines),
    )
    checksum += value

    checksum += run_batch_machines_analyze(1)
    batch_machines_analyze, value = measure(
        lambda: run_batch_machines_analyze(batch_machine_rounds),
        query_count=batch_machine_rounds * len(machines),
    )
    checksum += value

    return QuickQueryResult(
        get_capacity_50k=get_capacity_50k,
        analyze_50k=analyze_50k,
        machines_get_capacity=machines_get_capacity,
        machines_analyze=machines_analyze,
        batch_get_capacity_50k=batch_get_capacity_50k,
        batch_analyze_50k=batch_analyze_50k,
        batch_machines_get_capacity=batch_machines_get_capacity,
        batch_machines_analyze=batch_machines_analyze,
        checksum=checksum,
    )


def test_quick_query_perf_matches_csharp_scenarios():
    result = run_quick_query_perf()

    assert result.checksum == 28_681_706
    assert result.get_capacity_50k.avg_us > 0
    assert result.analyze_50k.avg_us > 0
    assert result.machines_get_capacity.avg_us > 0
    assert result.machines_analyze.avg_us > 0
    assert result.batch_get_capacity_50k.avg_us > 0
    assert result.batch_analyze_50k.avg_us > 0
    assert result.batch_machines_get_capacity.avg_us > 0
    assert result.batch_machines_analyze.avg_us > 0


if __name__ == "__main__":
    result = run_quick_query_perf()
    print(
        f"50k segments GetCapacityAt: total={result.get_capacity_50k.total_ms:.3f} ms, "
        f"avg={result.get_capacity_50k.avg_us:.3f} us/query"
    )
    print(
        f"50k segments Analyze: total={result.analyze_50k.total_ms:.3f} ms, "
        f"avg={result.analyze_50k.avg_us:.3f} us/query"
    )
    print(
        f"1000 machine timelines GetCapacityAt: total={result.machines_get_capacity.total_ms:.3f} ms, "
        f"avg={result.machines_get_capacity.avg_us:.3f} us/query"
    )
    print(f"1000-machine sweep GetCapacityAt ~= {result.machines_get_capacity.avg_us * 1000:.3f} us")
    print(
        f"1000 machine timelines Analyze: total={result.machines_analyze.total_ms:.3f} ms, "
        f"avg={result.machines_analyze.avg_us:.3f} us/query"
    )
    print(f"1000-machine sweep Analyze ~= {result.machines_analyze.avg_us * 1000:.3f} us")
    print(
        f"Batch 50k segments GetCapacitiesAt: total={result.batch_get_capacity_50k.total_ms:.3f} ms, "
        f"avg={result.batch_get_capacity_50k.avg_us:.3f} us/query"
    )
    print(
        f"Batch 50k segments AnalyzeMany: total={result.batch_analyze_50k.total_ms:.3f} ms, "
        f"avg={result.batch_analyze_50k.avg_us:.3f} us/query"
    )
    print(
        f"Batch 1000 machine timelines GetCapacitiesAt: total={result.batch_machines_get_capacity.total_ms:.3f} ms, "
        f"avg={result.batch_machines_get_capacity.avg_us:.3f} us/query"
    )
    print(
        f"Batch 1000-machine sweep GetCapacitiesAt ~= "
        f"{result.batch_machines_get_capacity.avg_us * 1000:.3f} us"
    )
    print(
        f"Batch 1000 machine timelines Analyze: total={result.batch_machines_analyze.total_ms:.3f} ms, "
        f"avg={result.batch_machines_analyze.avg_us:.3f} us/query"
    )
    print(f"Batch 1000-machine sweep Analyze ~= {result.batch_machines_analyze.avg_us * 1000:.3f} us")
    print(f"checksum={result.checksum}")
