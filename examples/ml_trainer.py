"""
Machine-learning training loop — realistic ML code with type violations.

Run:  cargo run -- check examples/ml_trainer.py
"""

from __future__ import annotations

from typing import Any, overload


# ── BSK-E0003: unannotated empty collections ─────────────────────────────────
_metric_history = []  # BSK-E0003: empty list, type unknown
_checkpoint_index = {}  # BSK-E0003: empty dict, type unknown


# ── BSK-E0001/E0002: untyped training functions ──────────────────────────────
def forward_pass(model, batch, device):  # BSK-E0001: three untyped params
    inputs, labels = batch
    logits = model(inputs.to(device))
    return logits  # BSK-E0002: no return type


def compute_loss(logits, labels, weights):  # BSK-E0001: three untyped params
    loss = ((logits - labels) ** 2).mean()
    return loss  # BSK-E0002: no return type


def backward_and_step(loss, optimizer):  # BSK-E0001: two untyped params
    loss.backward()
    optimizer.step()
    optimizer.zero_grad()  # BSK-E0002: no return type


# ── returns_compatibility: Any used in public interfaces without justification ────────────
def load_checkpoint(path: str) -> Any:  # returns_compatibility: Any return, no comment
    pass


def apply_augmentation(sample: Any, config: Any) -> Any:  # returns_compatibility ×3
    return sample


# ── assignment_compatibility: float hyperparameter assigned a string ────────────────────────
LEARNING_RATE: float = "1e-3"  # assignment_compatibility: str assigned to float
NUM_EPOCHS: int = 10.0  # assignment_compatibility: float assigned to int
DROPOUT: float = "0.5"  # assignment_compatibility: str assigned to float


# ── classes_override_2: subclass changes metric type ───────────────────────────────────
class Metric:
    name: str
    value: float
    higher_is_better: bool


class LossMetric(Metric):
    higher_is_better: str = "no"  # classes_override_2: str overrides bool


# ── names_undefined: reference to name not yet defined ─────────────────────────────
def get_default_optimizer() -> str:
    return DEFAULT_OPTIMIZER  # names_undefined: not yet assigned


DEFAULT_OPTIMIZER: str = "adam"


# ── names_unbound: epoch stats built conditionally, returned unconditionally ──────
def run_epoch(data: list[dict[str, float]], validate: bool) -> dict[str, float]:
    if validate:
        val_loss = sum(r["loss"] for r in data) / len(data)
    return {"val_loss": val_loss}  # names_unbound: val_loss may be unbound


# ── overloads_consistency: unannotated params make overloads identical ───────────────────
@overload
def decode_predictions(raw) -> list[int]: ...  # BSK-E0001: raw untyped


@overload
def decode_predictions(
    raw,
) -> list[int]: ...  # BSK-E0001 + overloads_consistency: duplicate


def decode_predictions(raw: list[float]) -> list[int]:
    return [round(x) for x in raw]


# ── dict_key_hashable: list literal as dict key ──────────────────────────────────────
def make_layer_index() -> dict[list[str], int]:
    return {["conv1", "conv2"]: 0}  # dict_key_hashable: list literal as key


# ── match_exhaustiveness: non-exhaustive match on optimizer name ────────────────────────
def build_optimizer(name: str, lr: float) -> str:
    match name:
        case "adam":
            return f"Adam(lr={lr})"
        case "sgd":
            return f"SGD(lr={lr})"
    # match_exhaustiveness: no wildcard branch — other values fall through silently


# ── BSK-E0025: override without @override ────────────────────────────────────
class BaseCallback:
    def on_epoch_end(self, epoch: int, metrics: dict[str, float]) -> None:
        pass


class EarlyStoppingCallback(BaseCallback):
    patience: int = 5

    def on_epoch_end(  # BSK-E0025: missing @override
        self, epoch: int, metrics: dict[str, float]
    ) -> None:
        pass
