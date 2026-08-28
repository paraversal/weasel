from itertools import groupby
from operator import attrgetter

from textual.app import ComposeResult
from textual.containers import VerticalScroll
from textual.widgets import Collapsible, Label, ListView, ListItem

from src.freshie import Freshie


class BatchGroup(Collapsible):
    def __init__(self, batch_id: str, freshies: list[Freshie], **kwargs):
        super().__init__(title=f"Batch {batch_id} ({len(freshies)})", **kwargs)
        self.freshies = freshies

    def compose(self) -> ComposeResult:
        yield ListView(
            *[
                ListItem(Label(f"{f.first_name} {f.last_name} — {f.email}"))
                for f in self.freshies
            ]
        )


class FreshiesByBatch(VerticalScroll):
    def __init__(self, freshies: list[Freshie], **kwargs):
        super().__init__(**kwargs)
        self.freshies = freshies

    def compose(self) -> ComposeResult:
        ordered = sorted(self.freshies, key=attrgetter("batch"))
        for batch_id, group in groupby(ordered, key=attrgetter("batch")):
            yield BatchGroup(batch_id, list(group))