These are synthetic, documentation-derived fixtures, not live captured responses. No request was sent.

Sources read 2026-11-01:
- https://docs.typesafe.ai/api/system-one (named answers, type tags, model and usage).
- https://laya.studio/v1/openapi (Question, DecisionRequest, Answer and DecisionResponse).
- https://laya.studio/glossary/systemone-protocol (choice probabilities and noul = P(true)).

The jev fixture uses illustrative probabilities and a deliberately illustrative resolved model id.
TypeSafe requires a request model and reports a resolved model that may differ from its alias.
Laya can route automatically, ignores unknown model ids, reports routing/confidence/action metadata,
and reports output_tokens = 0. Tests add these Laya fields to verify the shared parser tolerates them.
The request builder always sends the selected model unchanged; it does not implement routing or HTTP.
