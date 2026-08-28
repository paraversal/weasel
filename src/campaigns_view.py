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

from src.freshie import Freshie
from src.db import DEBUG_clear_table, get_all_campaigns, add_campaign, get_campaign_sent_info

class FreshieColumn(VerticalScroll):
    """One labeled, scrollable column of freshies."""

    def __init__(self, title: str, freshies: list[Freshie], **kwargs):
        super().__init__(**kwargs)
        self.title_text = title
        self.freshies = freshies

    def compose(self) -> ComposeResult:
        yield Static(self.title_text, classes="column-header")
        yield ListView(
            *[
                ListItem(Label(f"{f.first_name} {f.last_name}"))
                for f in self.freshies
            ]
        )


class CampaignSentStatusView(Horizontal):
    """Side-by-side sent / not-sent view for a campaign."""

    def __init__(self, sent: list[Freshie], not_sent: list[Freshie], **kwargs):
        super().__init__(**kwargs)
        self.sent = sent
        self.not_sent = not_sent

    def compose(self) -> ComposeResult:
        yield FreshieColumn("Sent", self.sent, id="sent-column")
        yield FreshieColumn("Not sent", self.not_sent, id="not-sent-column")

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
    
    def __init__(self, _id: int):
        super().__init__()
        self.campaign_id = _id
        
    def compose(self) -> ComposeResult:
        campaign_info = get_campaign_sent_info(self.campaign_id)
        self.sent = campaign_info[0]
        self.not_sent = campaign_info[1]
        yield CampaignSentStatusView(sent=self.sent, not_sent=self.not_sent)
        

class CampaignsView(Static):
    def __init__(self):
        super().__init__()

    campaigns = reactive(get_all_campaigns(), recompose=True)

    def compose(self) -> ComposeResult:
        yield HorizontalGroup(
            Button("Delete all!!!!", id="delete-campaigns", variant="error"), 
            Button("Add Campaign", id="new-campaign", variant="primary")
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
            self.campaigns = get_all_campaigns()

    def handle_new_campaign(self, result: NewCampaign | None) -> None:
        if result is not None:
            add_campaign(name=result.name, info=result.notes, text="")
            self.campaigns = get_all_campaigns()


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