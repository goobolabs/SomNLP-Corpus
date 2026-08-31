#!/usr/bin/env python3
"""Promote a trained tokenizer to the shipped v2 artifact plus a transformers-loadable dir.

Writes ``somali-bpe-v2.json`` and a ``v2/`` directory that ``AutoTokenizer.from_pretrained``
can load directly, so downstream training code needs no bespoke loading path.
"""

from __future__ import annotations

import argparse
import json
import logging
import shutil
from pathlib import Path

from tokenizers import Tokenizer

from common import (
    DEFAULT_HF_EXPORT_DIR,
    DEFAULT_TOKENIZER_V2,
    SPECIAL_TOKENS_V2,
    add_repo_path_arg,
    fail,
    resolve_under_repo,
    setup_logging,
    write_json,
)

BOS_TOKEN = EOS_TOKEN = UNK_TOKEN = "<|endoftext|>"
PAD_TOKEN = "<|pad|>"
MODEL_MAX_LENGTH = 8192


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Promote a tokenizer to the shipped v2 artifact.")
    add_repo_path_arg(parser)
    parser.add_argument("source", type=Path, help="Trained tokenizer JSON to promote")
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_TOKENIZER_V2,
        help="Shipped v2 artifact path",
    )
    parser.add_argument(
        "--hf-dir",
        type=Path,
        default=DEFAULT_HF_EXPORT_DIR,
        help="Directory for the transformers-loadable export",
    )
    parser.add_argument("--verbose", action="store_true", help="Enable debug logging")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    setup_logging(args.verbose)

    source = resolve_under_repo(args.repo_root, args.source)
    output = resolve_under_repo(args.repo_root, args.output)
    hf_dir = resolve_under_repo(args.repo_root, args.hf_dir)

    if not source.is_file():
        fail(f"Source tokenizer not found: {source}")

    tokenizer = Tokenizer.from_file(str(source))
    vocab_size = tokenizer.get_vocab_size()

    probe = "Soomaaliya waa dal ku yaal Geeska Afrika, oo leh taariikh hodan ah."
    if tokenizer.decode(tokenizer.encode(probe).ids, skip_special_tokens=False) != probe:
        fail(f"Refusing to promote {source}: it does not round-trip a plain Somali sentence.")

    output.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, output)
    logging.info("Promoted %s -> %s (vocab=%d)", source.name, output, vocab_size)

    hf_dir.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, hf_dir / "tokenizer.json")

    write_json(
        hf_dir / "special_tokens_map.json",
        {
            "bos_token": BOS_TOKEN,
            "eos_token": EOS_TOKEN,
            "unk_token": UNK_TOKEN,
            "pad_token": PAD_TOKEN,
        },
    )
    write_json(
        hf_dir / "tokenizer_config.json",
        {
            "tokenizer_class": "PreTrainedTokenizerFast",
            "model_max_length": MODEL_MAX_LENGTH,
            "bos_token": BOS_TOKEN,
            "eos_token": EOS_TOKEN,
            "unk_token": UNK_TOKEN,
            "pad_token": PAD_TOKEN,
            "clean_up_tokenization_spaces": False,
            "added_tokens_decoder": {
                str(tokenizer.token_to_id(token)): {
                    "content": token,
                    "special": True,
                    "normalized": False,
                    "lstrip": False,
                    "rstrip": False,
                    "single_word": False,
                }
                for token in SPECIAL_TOKENS_V2
            },
        },
    )
    logging.info("Wrote transformers export to %s", hf_dir)
    summary = {"artifact": str(output), "hf_dir": str(hf_dir), "vocab_size": vocab_size}
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
