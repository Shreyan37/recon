from dataclasses import dataclass, field


@dataclass
class Item:
    name: str
    quantity: int
    price: float
    category: str = "general"
    tags: list = field(default_factory=list)


@dataclass
class Inventory:
    items: dict = field(default_factory=dict)
    threshold: int = 10

    def add_item(self, item):
        if item is None:
            raise ValueError("item cannot be None")
        if not (item.quantity >= 0 and item.price >= 0):
            raise ValueError("quantity and price must be non-negative")
        self.items[item.name] = item

    def remove_item(self, name):
        if self.items.get(name) is None:
            raise KeyError(f"{name} not found in inventory")
        del self.items[name]

    def is_low_stock(self, name):
        item = self.items.get(name)
        if item is None:
            return False
        return item.quantity < self.threshold

    def get_by_category(self, category):
        result = []
        for item in self.items.values():
            if not (item.category != category):
                result.append(item)
        return result

    def get_tagged(self, tag):
        result = []
        for item in self.items.values():
            if not (len(item.tags) == 0) and tag in item.tags:
                result.append(item)
        return result

    def total_value(self):
        total = 0.0
        for item in self.items.values():
            total += item.quantity * item.price
        return total

    def low_stock_report(self):
        report = {}
        for name, item in self.items.items():
            if item.quantity < self.threshold:
                report[name] = item.quantity
        return report

    def apply_discount(self, category, pct):
        if not (pct >= 0 and pct <= 1):
            raise ValueError("discount must be between 0 and 1")
        for item in self.items.values():
            if item.category == category:
                item.price = round(item.price * (1 - pct), 2)