from webbrowser import get

import randfacts
from textual import on, work
from textual.reactive import reactive
from textual.app import App, ComposeResult, RenderResult
from textual.widget import Widget
from textual.widgets import Footer, Header, TabbedContent, Markdown, TabPane

from src.freshies_view import FreshiesView
from src.load import extract_freshies
from src.campaign import Campaign
from src.db import get_freshies, get_all_campaigns
from src.freshie import Freshie
from src.campaigns_view import CampaignsView
from textual_fspicker import FileOpen, Filters

class WeaselApp(App):
    """A Textual app to manage stopwatches."""

    CSS = """
        TabbedContent#campaign-tabs {
            border: none;
            height: 1fr;
        }

        TabbedContent#campaign-tabs > Tabs {
            background: $panel;
            border-bottom: solid $accent;
        }

        TabbedContent#campaign-tabs > ContentSwitcher {
            background: transparent;
        }
        TabbedContent#main-tabs {
            border: none;
        }

        TabbedContent#main-tabs > Tabs {
            background: $boost;
        }

        TabbedContent#main-tabs > ContentSwitcher {
            background: transparent;
        }
        """

    BINDINGS = [("o", "load_file", "Load email list"), ("q", "quit", "Quit"), ("d", "toggle_dark", "Toggle dark mode")]

    fact = reactive(randfacts.get_fact())
    active_campaign_id: reactive[str | None] = reactive(None)
    
    freshies_view = FreshiesView()
    freshie_amount = reactive(len(get_freshies()))

    def compose(self) -> ComposeResult:
        yield Header()
        with TabbedContent(initial="home", id="main-tabs"):
            with TabPane("Home", id="home"):
                yield Markdown("# Home - Welcome")
                yield Markdown("Weasel is your one stop shop for easy email campaign management.")
                yield Markdown(f"You are currently managing *{self.freshie_amount}* freshies. Wow!!")
                yield Markdown("## Did you know that...")
                yield Markdown(id="fact")
                
            with TabPane("Freshies", id="freshies"):
                yield self.freshies_view
            with TabPane("Campaigns", id="campaigns"):
                yield CampaignsView()
            with TabPane("Copies", id="copies"):
                yield Markdown("See all copies here !!!!")
        
        yield Footer()

    @on(TabbedContent.TabActivated)
    def on_switch_fact(self):
        fact_widget: Markdown = self.query_one("#fact", Markdown)
        fact = randfacts.get_fact().rstrip(".")
        fact = fact[0].lower() + fact[1:]
        fact_widget.update(f"... {fact}?!?!")

    @work
    async def action_load_file(self) -> None:
        filters = Filters((".xlsx", lambda p: p.suffix.lower() == ".xlsx"))
        if opened := await self.push_screen_wait(FileOpen(title="Select freshie table file", filters=filters)):
            number_extracted = extract_freshies(opened)
            self.notify(f"{number_extracted} freshies extracted!!! {"The family is groooowing!!" if number_extracted > 1 else "Well at least we didn't shrink!!!"}")
            updated_freshies_list = get_freshies()
            self.freshie_amount = len(updated_freshies_list)
            self.freshies_view.freshies = updated_freshies_list

    def quit(self) -> None:
        self.quit()

    def action_toggle_dark(self) -> None:
        """An action to toggle dark mode."""
        self.theme = (
            "textual-dark" if self.theme == "textual-light" else "textual-light"
        )