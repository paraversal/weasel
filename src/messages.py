from textual.message import Message

class DataInvalidated(Message):
    def __init__(self, dataset: str) -> None:
        self.dataset = dataset
        super().__init__()