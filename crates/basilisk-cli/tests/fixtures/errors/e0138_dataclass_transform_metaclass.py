from typing import dataclass_transform


@dataclass_transform(kw_only_default=True)
class ModelMeta(type):
    pass


class ModelBase(metaclass=ModelMeta):
    pass


class Customer(ModelBase, frozen=True):
    id: int


c = Customer(id=1)
c.id = 2  # E: frozen instance
