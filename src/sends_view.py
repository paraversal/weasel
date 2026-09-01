from itertools import groupby
from operator import attrgetter

from textual.app import ComposeResult
from textual.containers import VerticalScroll
from textual.reactive import reactive
from textual.widgets import Collapsible, DataTable, Markdown

from src.campaign import Campaign
from src.freshie import Freshie

from typing import TYPE_CHECKING, cast
if TYPE_CHECKING:
    from src.app import WeaselApp


class SendBatchGroup(Collapsible):
    DEFAULT_CSS = """
    BatchGroup DataTable {
        height: auto;
        max-height: 20;
        border: round $panel;
    }
    """

    def __init__(self, copy_id: str, campaign: Campaign, freshies: list[Freshie], **kwargs):
        super().__init__(title=f"Batch {campaign.id} ({len(freshies)})", **kwargs)
        self.freshies = freshies
        self.campaign = campaign
        self.copy_id = copy_id

    def compose(self) -> ComposeResult:
        table = DataTable()
        table.border_title = f"{self.campaign.name}, Copy {self.copy_id}"
        table.styles.border = ("round", "white")
        table.cursor_type = "row"
        table.add_columns("Name", "Email")
        table.add_rows(
            [(f"{f.first_name} {f.last_name}", f.email) for f in self.freshies]
        )
        yield table

class CampaignSendBatchesView(VerticalScroll):
    def __init__(self, **kwargs):
        super().__init__(**kwargs)

    all_sent_batches: reactive[dict[int, tuple[Campaign, list[Freshie]]]] = reactive(dict, recompose=True)

    def on_mount(self):
        self.weasel_app = cast("WeaselApp", self.app)
        self.watch(self.weasel_app, "campaign_sent_instances", self._on_source_changed)

    def _on_source_changed(self, all_sent_batches: dict[int, tuple[Campaign, list[Freshie]]]) -> None:
        self.all_sent_batches = all_sent_batches

    def compose(self) -> ComposeResult:
        if self.all_sent_batches == {}:
            yield Markdown("# No copies so far!")
        for copy_id, (campaign, group) in self.all_sent_batches.items():
            yield SendBatchGroup(copy_id=str(copy_id), campaign=campaign, freshies=list(group))