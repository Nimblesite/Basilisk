from typing import dataclass_transform


@dataclass_transform(kw_only_default=True)
class ModelBase:
    pass


class Customer(ModelBase, frozen=True):
    id: int


c = Customer(3)  # E: kw_only requires keyword args
c.id = 4  # E: frozen instance is immutable
