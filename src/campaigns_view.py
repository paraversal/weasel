from typing import TYPE_CHECKING, cast

from src.campaign import Campaign
from src.messages import DataInvalidated
if TYPE_CHECKING:
    from src.app import WeaselApp

from textual.app import App, ComposeResult
from textual import on, work
from textual.reactive import reactive
from textual.widget import Widget
from textual.containers import Horizontal, HorizontalGroup, Right, Vertical
from textual.widgets import Button, DataTable, Markdown, Static, TabPane, TabbedContent
from textual_fspicker import SelectDirectory
from dataclasses import dataclass
from textual.screen import ModalScreen
from textual.containers import Grid
from textual.widgets import Input, TextArea, Button, Label
from textual.app import ComposeResult
from textual.containers import Horizontal, VerticalScroll
from textual.widgets import Static, ListView, ListItem, Label
import pyperclip

from src.freshie import Freshie
from src.db import DEBUG_clear_table, add_campaign, get_campaign_sent_info

class FreshieColumn(VerticalScroll):
    DEFAULT_CSS = """
        DataTable {
            height: 1fr;
        }
        .copy-all-button {
            width: 1fr;
            margin-bottom: 1;
        }
    """

    def __init__(self, title, freshies, show_copy_all=False, **kwargs):
        super().__init__(**kwargs)
        self.border_title = title
        self.border_subtitle = f"{len(freshies)} freshies"
        if show_copy_all:
            self.styles.border = ("round", "red")
        else: 
            self.styles.border = ("round", "green")
        self.title_text = title
        self.freshies = freshies
        self.show_copy_all = show_copy_all

    def compose(self) -> ComposeResult:
        #yield Static(self.title_text, classes="column-header")
        if self.show_copy_all:
            yield Button("Copy all", id="copy-all", classes="copy-all-button")
        table = DataTable()
        table.add_columns("Name", "Email")
        table.add_rows(
            [(f"{f.first_name} {f.last_name}", f.email) for f in self.freshies]
        )
        yield table

    def on_data_table_row_selected(self, event: DataTable.RowSelected) -> None:
        # row selection replaces the per-row copy button — click a row to act on it
        row = event.data_table.get_row(event.row_key)
        self.app.notify(f"Copied: {row[1]}")


class CampaignSentStatusView(Horizontal):
    """Side-by-side sent / not-sent view for a campaign."""

    def __init__(self, sent: list[Freshie], not_sent: list[Freshie], **kwargs):
        super().__init__(**kwargs)
        self.sent = sent
        self.not_sent = not_sent

    def compose(self) -> ComposeResult:
        yield FreshieColumn("Sent to", self.sent, id="sent-column")
        yield FreshieColumn(
            "Not sent to", self.not_sent, show_copy_all=True, id="not-sent-column"
        )

class CampaignView(Static):
    DEFAULT_CSS = """
    CampaignSentStatusView {
    height: 1fr;
    }

    CampaignSentStatusView > FreshieColumn {
        width: 1fr;
        border: round $panel;
        padding: 1;
    }

    .column-header {
        text-style: bold;
        background: $boost;
        padding: 0 1;
    }
    """
    
    campaign_sent_to: reactive[list[Freshie]] = reactive(list, recompose=True)
    campaign_not_sent_to: reactive[list[Freshie]] = reactive(list, recompose=True)
    
    def __init__(self, _id: int):
        super().__init__()
        self.campaign_id = _id
    
    def on_mount(self):
        self.log("CampaignView mounted", self.campaign_id)
        self.weasel_app = cast("WeaselApp", self.app)
        self.campaign_sent_to = self.weasel_app.campaign_sent_infos[self.campaign_id][0]
        self.campaign_not_sent_to = self.weasel_app.campaign_sent_infos[self.campaign_id][1]
        self.watch(self.weasel_app, "campaign_sent_infos", self._on_source_changed, init=True)
    
    def _on_source_changed(self, campaigns: dict[int, tuple[list[Freshie], list[Freshie]]]) -> None:
        if campaigns is not None:
            self.campaign_sent_to = campaigns[self.campaign_id][0]
            self.campaign_not_sent_to = campaigns[self.campaign_id][1]
    
    def compose(self) -> ComposeResult:
        yield CampaignSentStatusView(sent=self.campaign_sent_to, not_sent=self.campaign_not_sent_to)
        

class CampaignsView(Static):
    def __init__(self):
        super().__init__()

    campaigns: reactive[list[Campaign]] = reactive(list, recompose=True)
    
    DEFAULT_CSS = """
    #campaign-toolbar {
        
        padding: 0 1;
        align: right middle;
        border-bottom: solid $panel-lighten-2;
        
    }

    #campaign-toolbar Button {
        margin-left: 1;
        min-width: 12;
    }

    #delete-campaigns {
        background: transparent;
        border: round $error;
        color: $error;
    }

    #delete-campaigns:hover {
        background: $error 20%;
        color: $text;
    }
    
    """
    
    def on_mount(self):
        self.weasel_app = cast("WeaselApp", self.app)
        self.campaigns = self.weasel_app.campaigns
        self.watch(self.weasel_app, "campaigns", self._on_source_changed)
        
    def _on_source_changed(self, campaigns: list[Campaign]) -> None:
        self.campaigns = campaigns

    def compose(self) -> ComposeResult:
        yield HorizontalGroup(
            Button("Delete all!!!!", id="delete-campaigns", variant="error"),
            Button("Add Campaign", id="new-campaign", variant="primary"),
            id="campaign-toolbar",
        )
        no_campaigns = len(self.campaigns) < 1
        if no_campaigns:
            yield Markdown("No campaigns.")
            return
        with TabbedContent(id="campaign-tabs"):
            for campaign in self.campaigns:
                with TabPane(campaign.name):
                    yield CampaignView(campaign.id)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "new-campaign":
            self.app.push_screen(AddCampaignScreen(), self.handle_new_campaign)
        if event.button.id == "delete-campaigns":
            DEBUG_clear_table("campaigns")
            self.post_message(DataInvalidated("campaigns"))

    def handle_new_campaign(self, result: NewCampaign | None) -> None:
        if result is not None:
            add_campaign(name=result.name, info=result.notes, text="")
            self.post_message(DataInvalidated("campaigns"))


@dataclass
class NewCampaign:
    name: str
    notes: str


class AddCampaignScreen(ModalScreen[NewCampaign | None]):
    """Popup for creating a new campaign. Dismisses with a NewCampaign, or None if cancelled."""

    DEFAULT_CSS = """
    AddCampaignScreen {
        align: center middle;
    }

    #dialog {
        grid-size: 2;
        grid-gutter: 1 2;
        grid-rows: auto auto 1fr auto;
        padding: 1 2;
        width: 60;
        height: 20;
        border: thick $accent;
        background: $surface;
    }

    #dialog > Label {
        column-span: 2;
    }

    #name {
        column-span: 2;
    }

    #notes {
        column-span: 2;
        height: 1fr;
    }
    """

    def compose(self) -> ComposeResult:
        with Grid(id="dialog"):
            yield Label("New campaign")
            yield Input(placeholder="Campaign name", id="name")
            yield TextArea(id="notes")
            yield Button("Cancel", id="cancel")
            yield Button("Save", id="save", variant="primary")

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "save":
            name = self.query_one("#name", Input).value
            notes = self.query_one("#notes", TextArea).text
            self.dismiss(NewCampaign(name=name, notes=notes))
        else:
            self.dismiss(None)