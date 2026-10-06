# Deepgram keyterm prompting

Use this file as the reference for custom words (#37, #38).
Researched on 2026-10-06. Check the [sources](#sources) again before you rely on a number.

A **key term** is a word or phrase that we ask Deepgram to recognize, for example a name, a product name or a technical word.
**Keyterm prompting** is the Deepgram feature that sends these terms with a request.

## Summary

- Nova-3 (our model) supports keyterm prompting. It does not support the older `keywords` feature.
- Send each term in its own `keyterm` query parameter.
- The limit is 500 tokens per request. If the list is longer, Deepgram returns an error.
- Deepgram recommends 20 to 50 terms. The marketing pages say "up to 100".
- It works for streaming and batch.
- It costs extra: $0.0013/min on Pay As You Go.

## How it works

Keyterm prompting does not retrain the model, and it does not replace words after the transcription.

- Nova-3 has a trained context mechanism. Deepgram calls it "in-context learning at inference time".
- The model gets the key terms as extra context together with the audio.
- When the audio is near a key term, the model gives that term a higher score.
- The model still decides from the audio. A key term does not force a word into the text.
- The idea is the same as a prompt for a large language model (LLM).

Deepgram gives this example. The audio has the word "nacho". Without key terms, the model writes "macho" (confidence 0.887). With the key term `nacho stack double crunch taco`, the model writes "nacho" (confidence 0.990).

Accuracy claims from Deepgram:

- Keyword recall rate (KRR) up to 90%. KRR is the share of key terms that the model writes correctly.
- One customer: the old model recognized 10% of veterinary terms. Nova-3 with key terms recognized 625% more.

These numbers come from Deepgram. Nobody has checked them independently.

### Difference from the older `keywords` feature

| | `keyterm` (new) | `keywords` (old) |
|---|---|---|
| Models | Nova-3, Flux | Nova-2, Nova-1, Enhanced, Base |
| How | Context for the model | Changes the score of a word by a number (boost) |
| Weight | None. Plain terms only | `keywords=term:2`. Can be negative to suppress a word |
| Limit | 500 tokens | 100 keywords |
| Format | Keeps the case and punctuation of the term | Does not |

We use Nova-3, so we must use `keyterm`. Do not add a weight such as `:2`.

## Request format

Correct:

```
?model=nova-3&keyterm=Deepgram&keyterm=Rawas
?keyterm=customer%20service
?keyterm=customer+service
```

Wrong:

```
?keyterm=Deepgram:2              (no weights)
?keyterm=Deepgram,Rawas          (no commas)
?keyterm=Deepgram;Rawas          (no semicolons)
```

- Repeat the parameter for each term.
- URL-encode each term. A space becomes `%20` or `+`.
- The same parameters work for the streaming WebSocket URL and for the batch `POST /v1/listen` URL.

### Streaming: change the list during a session

- In a streaming session, a `Configure` message can replace the list without a new connection.
- The new list replaces the full old list. An empty list removes all terms.
- The 500 token limit also applies to this message.
- Batch requests cannot change the list. Send it with the request.

We do not need this. Our app opens a new connection for each dictation, so it can send the current list in the URL.

## Limits

| Limit | Value |
|---|---|
| Tokens per request | 500. More returns an error |
| Recommended number of terms | 20 to 50 |
| Languages | Monolingual and multilingual Nova-3 (`language=multi` since 2025-11-26) |

Deepgram does not say how it counts tokens. A token is a part of a word. A short common word is about one token. A rare name can be several tokens. For #37:

- Show a warning before the list gets near the limit.
- If Deepgram returns an error because the list is too long, send the audio again without key terms. Do not lose the user's speech.
- The list stays the same, so every later dictation would fail too. Tell the user once, and send no key terms until `words.txt` changes.

## Best practices

Add:

- Names of people, companies and products: `Deepgram`, `iPhone`, `Dr. Smith`.
- Technical words and words from your work.
- Phrases of more than one word that you often say.
- Common nouns in lowercase.

Do not add:

- Common words, such as `the`, `and`, `is`. The model already knows them.
- General words that can mean many things.
- Too many terms. More terms can make other words less accurate.

Case and spelling:

- Deepgram keeps the case and punctuation of the key term. Write the term the way you want to see it, for example `GitHub`, not `github`.
- A word at the start of a sentence can get a capital letter, whatever the key term says.
- Use one spelling for a term. Do not add the same term with different cases.

## Cost

| Plan | Nova-3 streaming | Nova-3 batch | Keyterm add-on |
|---|---|---|---|
| Pay As You Go | $0.0048/min (sale, normal $0.0077) | $0.0043/min | +$0.0013/min |
| Growth | $0.0042/min (sale, normal $0.0065) | $0.0036/min | +$0.0012/min |

Prices from deepgram.com/pricing on 2026-10-06 (monolingual).
At the normal streaming price, key terms add about 17% to the cost per minute.
We think the add-on is charged only for requests that include `keyterm`. Deepgram does not say this clearly. With no `words.txt`, the app sends no key terms.

## What this means for our app

- Use `keyterm`, not `keywords` (#37).
- Send the list in the URL of each request, streaming and batch.
- Keep the case from `words.txt`.
- Handle the 500 token error without losing the dictation. Do not repeat a request that failed for each dictation (#37).
- In the window (#38), show a count and a warning near the limit. Suggest 20 to 50 terms.

## Sources

- [Deepgram docs: Keyterm Prompting](https://developers.deepgram.com/docs/keyterm)
- [Deepgram docs: Keywords](https://developers.deepgram.com/docs/keywords)
- [Deepgram: Introducing Nova-3](https://deepgram.com/learn/introducing-nova-3-speech-to-text-api)
- [Deepgram changelog, 2025-11-26: key terms for multilingual Nova-3](https://developers.deepgram.com/changelog/2025/11/26)
- [Deepgram pricing](https://deepgram.com/pricing)
