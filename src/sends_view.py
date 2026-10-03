from fuzzysearch import find_near_matches
from textual import on
from textual.app import ComposeResult
from textual.containers import Vertical, VerticalScroll
from textual.reactive import reactive
from textual.widgets import Collapsible, DataTable, Input, Markdown

from src.campaign import Campaign
from src.freshie import Freshie

from typing import TYPE_CHECKING, cast
if TYPE_CHECKING:
    from src.app import WeaselApp


def matches(term: str, haystack: str) -> bool:
    term = term.strip().lower()
    if not term or term in haystack:
        return True
    if len(term) < 4:
        return False
    return bool(find_near_matches(term, haystack, max_l_dist=1))


class SendBatchGroup(Vertical):
    DEFAULT_CSS = """
    SendBatchGroup { height: auto; }
    SendBatchGroup DataTable { height: auto; max-height: 20; border: round $panel; }
    """

    global_term: reactive[str] = reactive("")
    local_term: reactive[str] = reactive("")    

    def __init__(self, copy_id: str, campaign: Campaign, freshies: list[Freshie], **kwargs):
        super().__init__(**kwargs)
        self.copy_id = copy_id
        self.campaign = campaign
        self.freshies = freshies
        
        self._haystacks = [f"{f.first_name} {f.last_name} {f.email}".lower() for f in freshies]

    def _title(self, shown: int) -> str:
        total = len(self.freshies)
        count = f"{shown}/{total}" if shown != total else str(total)
        return f"{self.campaign.name}, Batch {self.copy_id} ({count})"

    def compose(self) -> ComposeResult:
        with Collapsible(title=self._title(len(self.freshies))):
            if len(self.freshies) > 7:
                yield Input(placeholder="Search this copy...", classes="local-search")
            table = DataTable(cursor_type="row")
            table.border_title = f"{self.campaign.name}, Copy {self.copy_id}"
            yield table

    def on_mount(self) -> None:
        self.query_one(DataTable).add_columns("Name", "Email")
        self._refilter()

    @on(Input.Changed, ".local-search")
    def _local_changed(self, event: Input.Changed) -> None:
        event.stop()
        self.local_term = event.value

    def watch_global_term(self) -> None:
        self._refilter()

    def watch_local_term(self) -> None:
        self._refilter()

    def _refilter(self) -> None:
        if not self.is_mounted:
            return
        global_hits = [(f, h) for f, h in zip(self.freshies, self._haystacks)
                       if matches(self.global_term, h)]
        rows = [(f"{f.first_name} {f.last_name}", f.email)
                for f, h in global_hits if matches(self.local_term, h)]

        table = self.query_one(DataTable)
        table.clear()
        table.add_rows(rows)
        self.query_one(Collapsible).title = self._title(len(rows))
        self.display = bool(global_hits)


class CampaignSendBatchesView(VerticalScroll):
    global_search_term: reactive[str] = reactive("")
    all_sent_batches: reactive[dict[int, tuple[Campaign, list[Freshie]]]] = reactive(dict, recompose=True)

    def on_mount(self) -> None:
        self.weasel_app = cast("WeaselApp", self.app)
        self.watch(self.weasel_app, "campaign_sent_instances", self._on_source_changed)

    def _on_source_changed(self, batches: dict[int, tuple[Campaign, list[Freshie]]]) -> None:
        self.all_sent_batches = batches

    def compose(self) -> ComposeResult:
        if not self.all_sent_batches:
            yield Markdown("# No copies so far!")
            return
        yield Input(placeholder="Search all copies...",
                    value=self.global_search_term, id="global-search")
        for copy_id, (campaign, group) in reversed(self.all_sent_batches.items()):
            yield SendBatchGroup(
                copy_id=str(copy_id), campaign=campaign, freshies=list(group), id=f"copy-{copy_id}"
            ).data_bind(global_term=CampaignSendBatchesView.global_search_term)

    @on(Input.Changed, "#global-search")
    def _global_changed(self, event: Input.Changed) -> None:
        self.global_search_term = event.value