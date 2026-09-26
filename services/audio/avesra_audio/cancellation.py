"""Private cooperative unwind, never an inference result or authority."""


class Cancelled(Exception):
    pass


def check(cancelled):
    if cancelled():
        raise Cancelled("request_cancelled")
