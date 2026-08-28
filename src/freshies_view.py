from textual.app import ComposeResult
from textual.containers import Container
from textual.reactive import reactive
from textual.widget import Widget
from textual.widgets import DataTable
from textual.app import ComposeResult

from src.db import get_freshies
from src.freshie import Freshie

class FreshiesView(Widget):
    DEFAULT_CSS = """
    FreshiesView {
        height: 1fr;
        width: 1fr;
    }
    
    DataTable {
        height: 100%;
    }

    DataTable > .datatable--header {
        background: $primary;
        color: $text;
        text-style: bold;
    }

    DataTable > .datatable--cursor {
        background: $accent;
    }
    """

    def __init__(self):
        super().__init__()

    freshies: reactive[list[Freshie]] = reactive(list, init=False)

    def compose(self) -> ComposeResult:
        yield Container(
            DataTable(id="freshie_table", zebra_stripes=True, cursor_type="row"),
        )

    def on_mount(self) -> None:
        table = self.query_one("#freshie_table", DataTable)
        table.add_columns(
            "First Name",
            "Last Name",
            "Email",
            "Date Added",
        )
        self.freshies = get_freshies()
    
    def watch_freshies(self, freshies: list[Freshie]) -> None:
        self.populate_table()

    def populate_table(self) -> None:
        table = self.query_one("#freshie_table", DataTable)
        table.clear()
        for f in self.freshies:
            table.add_row(
                f.first_name,
                f.last_name,
                f.email,
                f.date_added,
            )
    
    def action_refresh_table(self) -> None:
        self.freshies = get_freshies()