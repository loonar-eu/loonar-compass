"""Independent judge of a manual test case: gets only the test case (objective, precondition, steps with expected
results) and the evidence collected by whoever executed it (texts, logs, screenshots), never the code or the
executor's opinion, and returns PASS / FAIL / BLOCKED with a short reason per step. Uses the cheapest Claude model."""
import base64
import json
import mimetypes
from pathlib import Path

import anthropic

MODEL = "claude-haiku-5-5"
TEXT_LIMIT = 6000  # characters per evidence file; logs are cut at the end, where results usually are
IMAGE_TYPES = {".png", ".jpg", ".jpeg", ".gif", ".webp"}

SYSTEM = """You are an independent software tester judging one manual test case of a desktop application.
You get the test case (objective, precondition, steps with their expected results) and the evidence the executor
collected. Judge only by the evidence: a step passes when the evidence shows its expected result; it fails when the
evidence shows something else; it is blocked when the evidence does not show it either way (missing, unreadable).
Be strict about facts in the expected result (numbers, names, messages), lenient about wording and formatting;
"…" in an expected result stands for any value (an id, a count, a name). Something extra on the screen that the
expected result does not mention (another window still open, more items) does not fail a step.
The overall verdict is FAIL if any step fails, otherwise BLOCKED if any step is blocked, otherwise PASS.
Write the reasons in the language of the test case, one or two short sentences, naming what the evidence shows."""

SCHEMA = {
    "type": "object",
    "properties": {
        "verdict": {"type": "string", "enum": ["PASS", "FAIL", "BLOCKED"]},
        "reason": {"type": "string"},
        "steps": {"type": "array", "items": {"type": "object", "properties": {
            "index": {"type": "integer"},
            "verdict": {"type": "string", "enum": ["PASS", "FAIL", "BLOCKED"]},
            "note": {"type": "string"}}, "required": ["index", "verdict", "note"], "additionalProperties": False}},
    },
    "required": ["verdict", "reason", "steps"],
    "additionalProperties": False,
}


def _case_text(case):
    lines = [f"Test case {case['key']}: {case['name']}"]
    if case.get("objective"):
        lines.append(f"Objective: {case['objective']}")
    if case.get("precondition"):
        lines.append(f"Precondition: {case['precondition']}")
    for i, step in enumerate(case["steps"], 1):
        lines.append(f"Step {i}: {step['action']}")
        if step.get("data"):
            lines.append(f"  Test data: {step['data']}")
        lines.append(f"  Expected: {step['expected']}")
    return "\n".join(lines)


def _evidence_blocks(evidence_dir):
    blocks = []
    for path in sorted(Path(evidence_dir).iterdir()):
        if path.suffix.lower() in IMAGE_TYPES:
            media = mimetypes.guess_type(path.name)[0] or "image/png"
            blocks.append({"type": "text", "text": f"Evidence (screenshot): {path.name}"})
            blocks.append({"type": "image", "source": {"type": "base64", "media_type": media, "data": base64.standard_b64encode(path.read_bytes()).decode()}})
        elif path.is_file():
            text = path.read_text(encoding="utf-8", errors="replace")
            if len(text) > TEXT_LIMIT:
                text = "[…beginning left out…]\n" + text[-TEXT_LIMIT:]
            blocks.append({"type": "text", "text": f"Evidence ({path.name}):\n{text}"})
    return blocks or [{"type": "text", "text": "No evidence was collected."}]


def judge(case, evidence_dir, api_key):
    """Returns ({"verdict", "reason", "steps": [...]}, usage) for one test case."""
    client = anthropic.Anthropic(api_key=api_key)
    content = [{"type": "text", "text": _case_text(case)}] + _evidence_blocks(evidence_dir)
    response = client.messages.create(
        model=MODEL, max_tokens=6000, system=SYSTEM,
        output_config={"effort": "medium", "format": {"type": "json_schema", "schema": SCHEMA}},
        messages=[{"role": "user", "content": content}],
    )
    if response.stop_reason in ("refusal", "max_tokens"):
        return {"verdict": "BLOCKED", "reason": f"The judge gave no result ({response.stop_reason}).", "steps": []}, response.usage
    text = next(b.text for b in response.content if b.type == "text")
    result = json.loads(text)
    # The overall verdict follows from the steps, so a reason and a verdict can never contradict each other.
    verdicts = [s["verdict"] for s in result["steps"]]
    if verdicts:
        result["verdict"] = "FAIL" if "FAIL" in verdicts else "BLOCKED" if "BLOCKED" in verdicts else "PASS"
    return result, response.usage
