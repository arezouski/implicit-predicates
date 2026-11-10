"""The Thingi10K models: their environment, metadata and OFF files.

The dataset package needs Python 3.13 and third-party packages, so it runs in
its own environment under ``corpus/data/venv/``, created with ``uv``.
``dataset.py`` is run there as a subprocess; this module only uses the
standard library.
"""

import os
import shutil
import subprocess
import sys
from pathlib import Path

from reference import DATA_DIR, ReferenceError

TOOLS_DIR = Path(__file__).resolve().parent
REQUIREMENTS = TOOLS_DIR / "requirements.txt"
DATASET_SCRIPT = TOOLS_DIR / "dataset.py"
VENV_DIR = DATA_DIR / "venv"
VENV_PYTHON = VENV_DIR / "bin" / "python"
CACHE_DIR = DATA_DIR / "thingi10k"
METADATA = CACHE_DIR / "metadata.csv"
MODELS_DIR = DATA_DIR / "models"
PYTHON_VERSION = "3.13"


def _run(command, env=None):
    if subprocess.run(command, env=env).returncode != 0:
        raise ReferenceError(f"command failed: {' '.join(map(str, command))}")


def ensure_venv():
    """Create the environment of the dataset package, if missing, and install
    the pinned requirements into it."""
    if shutil.which("uv") is None:
        raise ReferenceError("uv was not found in PATH")
    if not VENV_PYTHON.exists():
        _run(["uv", "venv", "--python", PYTHON_VERSION, str(VENV_DIR)])
    _run(["uv", "pip", "install", "--quiet", "--python", str(VENV_PYTHON),
          "-r", str(REQUIREMENTS)])


def _dataset(*args):
    """Run ``dataset.py`` in the environment, keeping every cache under
    ``corpus/data/``."""
    env = dict(os.environ, HF_HOME=str(CACHE_DIR / "hf"))
    # -I: isolated mode, so nothing from the current directory or the
    # environment's PYTHON* variables is imported.
    _run([str(VENV_PYTHON), "-I", str(DATASET_SCRIPT), *map(str, args)], env=env)


def fetch_metadata():
    """Download the dataset if needed and write ``METADATA``."""
    ensure_venv()
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    _dataset("metadata", CACHE_DIR, METADATA)


def model_path(file_id):
    """The OFF file of a model."""
    return MODELS_DIR / f"{file_id}.off"


def export(file_ids):
    """Write the OFF file of every model in ``file_ids`` that has none yet."""
    missing = sorted({i for i in file_ids if not model_path(i).exists()})
    # Keep each command line well under the system limit.
    for start in range(0, len(missing), 500):
        _dataset("export", CACHE_DIR, MODELS_DIR, *missing[start:start + 500])


if __name__ == "__main__":
    sys.exit("run corpus.py instead")
