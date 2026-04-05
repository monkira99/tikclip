import pytest
from fastapi.testclient import TestClient

from src.app import create_app


@pytest.fixture
def client():
    app = create_app()
    return TestClient(app)
