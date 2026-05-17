import fleet_batch_snapshot
import json_schedule_template
import manufacturing_capacity_slot
import overnight_shift
import sla_due_date


EXAMPLES = [
    sla_due_date,
    manufacturing_capacity_slot,
    fleet_batch_snapshot,
    json_schedule_template,
    overnight_shift,
]


def main():
    for index, module in enumerate(EXAMPLES):
        if index:
            print()
        module.main()


if __name__ == "__main__":
    main()
