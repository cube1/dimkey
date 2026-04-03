"""Dimkey E2E pytest fixtures"""

import os
import subprocess
import signal
import time
from pathlib import Path

import pytest
from playwright.sync_api import sync_playwright

PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent


@pytest.fixture(scope="session")
def browser():
    """会话级浏览器实例"""
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        yield browser
        browser.close()


@pytest.fixture
def page(browser):
    """每个测试独立的页面"""
    context = browser.new_context(
        viewport={"width": 1200, "height": 800},
    )
    pg = context.new_page()

    url = os.environ.get("DIMKEY_TEST_URL", "http://localhost:1420")
    pg.goto(url)
    pg.wait_for_load_state("networkidle")

    yield pg

    pg.close()
    context.close()


@pytest.fixture
def sample_files():
    """测试样本文件路径"""
    fixtures_dir = Path(__file__).parent.parent / "fixtures"
    return {
        "xlsx": str(fixtures_dir / "sample.xlsx"),
        "csv": str(fixtures_dir / "sample.csv"),
        "docx": str(fixtures_dir / "sample.docx"),
        "txt": str(fixtures_dir / "sample.txt"),
    }
