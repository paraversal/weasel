class Campaign():
    def __init__(self, name: str, text: str, info: str, sent_to: list[int], _id: int):
        self.id = _id
        self.name = name
        self.text = text
        self.info = info
        self.sent_to = sent_to

    def __repr__(self):
        return f"Campaign: '{self.name}', id: {self.id}"