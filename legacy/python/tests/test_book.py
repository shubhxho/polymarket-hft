from hft.core.book import L2Book


def test_snapshot_and_level():
    b = L2Book("tok", tick=0.01)
    b.apply_snapshot(
        bids=[{"price": "0.61", "size": "40"}, {"price": "0.60", "size": "10"}],
        asks=[{"price": "0.63", "size": "25"}, {"price": "0.64", "size": "80"}],
    )
    top = b.top()
    assert top is not None
    assert abs(top.bid - 0.61) < 1e-9
    assert abs(top.ask - 0.63) < 1e-9
    assert abs(top.spread - 0.02) < 1e-9
    b.apply_level(0.61, 0, "BUY")
    top = b.top()
    assert abs(top.bid - 0.60) < 1e-9
    assert b.ask_size_upto(0.64) == 105
