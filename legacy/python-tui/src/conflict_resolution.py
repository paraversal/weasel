from dataclasses import dataclass
from textual.screen import ModalScreen

from src.freshie import Freshie

@dataclass
class ConflictType_Removed:
    

@dataclass
class Conflict:
    db_freshie: Freshie
    conflict: Literall[]

@dataclass
class ConflictResolution:
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