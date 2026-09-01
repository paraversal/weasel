from itertools import groupby
from operator import attrgetter

from textual.app import ComposeResult
from textual.containers import VerticalScroll
from textual.reactive import reactive
from textual.widgets import Collapsible, DataTable

from src.freshie import Freshie

from typing import TYPE_CHECKING, cast
if TYPE_CHECKING:
    from src.app import WeaselApp


class BatchGroup(Collapsible):
    DEFAULT_CSS = """
    BatchGroup DataTable {
        height: auto;
        max-height: 20;
        border: round $panel;
    }
    """

    def __init__(self, batch_id: str, freshies: list[Freshie], **kwargs):
        super().__init__(title=f"Batch {batch_id} ({len(freshies)})", **kwargs)
        self.freshies = freshies
        self.batch_id = batch_id

    def compose(self) -> ComposeResult:
        table = DataTable()
        table.border_title = f"Batch {self.batch_id}"
        table.styles.border = ("round", "white")
        table.cursor_type = "row"
        table.add_columns("Name", "Email")
        table.add_rows(
            [(f"{f.first_name} {f.last_name}", f.email) for f in self.freshies]
        )
        yield table


class FreshiesByBatch(VerticalScroll):
    def __init__(self, **kwargs):
        super().__init__(**kwargs)

    freshies: reactive[list[Freshie]] = reactive(list, recompose=True)

    def on_mount(self):
        self.weasel_app = cast("WeaselApp", self.app)
        self.watch(self.weasel_app, "freshies", self._on_source_changed)

    def _on_source_changed(self, freshies: list[Freshie]) -> None:
        self.freshies = freshies

    def compose(self) -> ComposeResult:
        ordered = sorted(self.freshies, key=attrgetter("batch"), reverse=True)
        for batch_id, group in groupby(ordered, key=attrgetter("batch")):
            yield BatchGroup(batch_id, list(group))