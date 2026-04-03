"""根级 conftest — 将 e2e/ 目录加入 sys.path，使 utils 包可直接导入"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
