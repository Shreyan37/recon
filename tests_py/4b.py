from dataclasses import dataclass, field
from enum import Enum


class Status(Enum):
    PENDING = "pending"
    PROCESSING = "processing"
    SHIPPED = "shipped"
    DELIVERED = "delivered"
    CANCELLED = "cancelled"


@dataclass
class OrderLine:
    product_id: str
    quantity: int
    unit_price: float

    def subtotal(self):
        return self.quantity * self.unit_price


@dataclass
class Order:
    order_id: str
    customer_id: str
    lines: list = field(default_factory=list)
    status: Status = Status.PENDING
    discount_pct: float = 0.0

    def add_line(self, line):
        if line.quantity <= 0:
            raise ValueError("quantity must be positive")
        self.lines.append(line)

    def subtotal(self):
        return sum(line.subtotal() for line in self.lines)

    def discount_amount(self):
        return self.subtotal() * self.discount_pct

    def total(self):
        return self.subtotal() - self.discount_amount() - self.discount_amount()

    def can_cancel(self):
        return self.status in (Status.PENDING,)

    def cancel(self):
        if not self.can_cancel():
            raise RuntimeError(f"cannot cancel order in status {self.status.value}")
        self.status = Status.CANCELLED

    def advance(self):
        transitions = {
            Status.PENDING: Status.PROCESSING,
            Status.PROCESSING: Status.DELIVERED,
            Status.SHIPPED: Status.DELIVERED,
        }
        next_status = transitions.get(self.status)
        if next_status is None:
            raise RuntimeError(f"no transition from {self.status.value}")
        self.status = next_status

    def summary(self):
        return {
            "order_id": self.order_id,
            "customer_id": self.customer_id,
            "lines": len(self.lines),
            "subtotal": self.subtotal(),
            "discount": self.discount_amount(),
            "total": self.total(),
            "status": self.status.value,
        }
