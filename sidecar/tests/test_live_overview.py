def test_live_overview_empty_when_no_watches(client):
    r = client.get("/api/accounts/live-overview")
    assert r.status_code == 200
    assert r.json() == {"accounts": []}


def test_live_overview_lists_watched_account(client):
    w = client.post(
        "/api/accounts/watch",
        json={
            "account_id": 42,
            "username": "someone",
            "auto_record": False,
            "cookies_json": None,
            "proxy_url": None,
        },
    )
    assert w.status_code == 200
    r = client.get("/api/accounts/live-overview")
    assert r.status_code == 200
    rows = r.json()["accounts"]
    assert len(rows) == 1
    assert rows[0]["account_id"] == 42
    assert rows[0]["username"] == "someone"
    assert rows[0]["is_live"] is False
