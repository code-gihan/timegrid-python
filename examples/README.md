# TimeGrid Python Examples

Install the package, then run any example directly:

```bash
pip install timegrid
python examples/sla_due_date.py
python examples/manufacturing_capacity_slot.py
python examples/fleet_batch_snapshot.py
python examples/json_schedule_template.py
python examples/overnight_shift.py
```

From a local checkout, run all examples with the development virtualenv:

```bash
.\.venv\Scripts\python.exe examples\run_all.py
```

## What These Cover

| Example | Scenario |
| --- | --- |
| `sla_due_date.py` | Add business hours while skipping lunch, nights, weekends, and closures. |
| `manufacturing_capacity_slot.py` | Model downtime and capacity boosts, then find the first slot with enough capacity. |
| `fleet_batch_snapshot.py` | Query many compiled machine timelines at one timestamp with `TimeGridTimelineBatch`. |
| `json_schedule_template.py` | Serialize reusable source schedule definitions as JSON and apply per-machine exceptions. |
| `overnight_shift.py` | Handle shifts that cross midnight, with a break inside the overnight window. |
