"""Goose SDK demo: build a declarative provider and stream a completion."""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent.parent / "generated"))

from aaif_goose import (  # noqa: E402
    DeclarativeProvider,
    MessageRole,
    ProviderMessage,
    ProviderModelConfig,
)


def main() -> None:
    provider_json = (HERE.parent.parent.parent / "goose-providers" / "examples" / "deepseek.json").read_text()
    provider = DeclarativeProvider.from_json(provider_json)
    model = ProviderModelConfig(
        model_name="deepseek-v4-flash",
        context_limit=None,
        temperature=None,
        max_tokens=None,
        toolshim=False,
        toolshim_model=None,
        request_params_json=None,
        reasoning=None,
    )
    messages = [ProviderMessage(role=MessageRole.USER, text="what is the capital of France?")]
    stream = provider.stream(
        model,
        "You are a knowledgable geography expert",
        messages,
    )

    print(f"{provider.name()}:")
    while chunk := stream.next():
        if chunk.text:
            print(chunk.text, end="")
        if chunk.usage_json:
            usage = json.loads(chunk.usage_json)
            print(f"\nusage: {usage}")
    print()


if __name__ == "__main__":
    main()
