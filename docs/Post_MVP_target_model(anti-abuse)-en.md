# Post_MVP_target_model(anti-abuse)-en.md — Target post-MVP anti-abuse economics model

**Languages:** English (this file) · [Русский — Post_MVP_target_model(anti-abuse).md](Post_MVP_target_model(anti-abuse).md)

**Date:** 2026-05-25  
**Status:** Concept note / post-MVP target model  
**Purpose:** Capture the economic and architectural framework of the anti-abuse model beyond the current MVP roadmap.  
**Related docs:** `CONCEPT_ROADMAP.md`, `CONCEPT_PROGRESS.md`, `../DRAFT_WHITEPAPER.md`, `OFFCHAIN_STUB.md`, `offchain-batch.md`  
**Translation note:** English translation of the Russian concept note, 2026-06-02

---

## 1. Why a separate document is needed

This topic should not enter the current MVP roadmap as a mandatory deliverable. It touches mark economics, offchain scaling, the product model for free services, anti-spam/anti-bot protection, trust/reputation semantics, and potentially sensitive regulatory questions—all at once.

At the same time, the topic is too important to leave as background intuition only. A separate conceptual note is needed that:

- preserves the connections already developed between offchain optimization and anti-abuse economics;
- records that a free service almost always means redistributed costs, not their absence;
- sets a target post-MVP framework without prematurely promising a specific protocol implementation;
- helps preserve the economic logic in future RFCs/ADRs on UC-1, UC-2, and related directions.

Otherwise there is a risk of two extremes:

- either anti-abuse is discussed only as technical rate-limiting and loses its economic meaning;
- or the economic idea starts to sound prematurely like a finished product or a promise of mandatory bonding for all services.

---

## 2. Core thesis

Free digital services are not actually free in reality. Their costs for spam, mass automation, disposable-account registration, moderation, manual incident review, reputation filters, and anti-spam model inference are always borne by someone.

Usually these costs are redistributed among:

- the service operator;
- an advertising or subscription model;
- honest users;
- infrastructure and security teams;
- service quality, which degrades under abuse pressure.

In the post-MVP horizon, PWM can be viewed not as a mechanism that magically eliminates these costs, but as a protocol layer that helps:

- make abuse costs more measurable;
- limit their uncontrolled scaling;
- structure the economics of secondary protective measures;
- in the most significant scenarios, partially return costs to the abuser.

The key starting point:

> Free and subsidized almost always means not the absence of cost, but its redistribution. PWM's role in the target anti-abuse model is to help manage these costs and, in critical cases, reflect them back onto the abuser.

---

## 3. Why this topic is directly tied to offchain scaling

If a significant share of PWM is held by businesses and in staking, a large part of mark emission will also arise at large operators. In such a scenario the network naturally tends not toward a retail model where millions of users individually buy PWM, stake it, and interact with the network directly, but toward an aggregated model.

The service operator:

- holds the main stake;
- produces marks;
- batches activity off-chain;
- decides what share of the mark flow to use for its own needs;
- what share to grant users as quotas, limits, or subsidized access.

This makes offchain batching and operator-side aggregation not merely a technical optimization, but an economic damper for the entire mark model.

### 3.1. Why this matters for perception of high mark emission

If mark emission is viewed in isolation, it may be perceived as too large or even hyperinflationary. But with operator offchain aggregation, a significant share of that emission does not have to enter the world as a chaotic surplus of a useless asset.

It can be absorbed by application services, where marks become:

- send quotas;
- anti-spam limits;
- API access resources;
- proof-of-send / proof-of-access filters;
- an internal budget for anti-abuse policy.

In such a model, excess mark emission is softened not by promises of scarcity, but by service consumption and aggregated offchain management.

In other words:

> offchain aggregation can act as a natural damper on high mark emission: excess marks are absorbed by application quotas rather than necessarily turning into market surplus that undermines the utility narrative.

---

## 4. The basic subsidized contour and its limits

The softest and least regulatorily toxic model looks like this:

- the service operator holds stake itself;
- the operator produces marks itself;
- the user is not required to buy PWM or understand staking;
- the operator grants the user a limited volume of capabilities as part of a free or conditionally free service.

For the user this may look like free mail, free basic API access, or another service where part of the anti-abuse cost is covered by the operator itself, while payback is achieved indirectly:

- through advertising;
- premium tier;
- upsell;
- lower spam costs;
- lower bot traffic;
- reduced load on abuse-ops.

### 4.1. Why subsidy alone is not enough

Subsidizing mark quotas does limit abuser throughput, but it does not fully solve the problem of disposable-account factories.

If account registration is free and costs almost nothing, an attacker can scale an attack through a large number of identities. The economic barrier then shifts to the send level, but not to account creation.

This means a simple model such as:

- "a new account gets a little free mark quota";
- "only a small volume of messages can be sent per day";

reduces the scale of a spam storm but does not eliminate the industry of mass account registration.

Therefore subsidy is useful as a first layer of protection, but by itself it is not a complete anti-Sybil architecture.

---

## 5. Why classic CAPTCHAs are no longer a sufficient barrier

The experience of mass mail services since the 2000s showed that with a strong enough incentive, attackers quickly move account registration to botnets, distributed automation, and proxy infrastructure.

CAPTCHAs worked for a long time as a weak friction layer, but not as a fundamental economic barrier. With the development of AI, solver factories, and automated pipelines, their effectiveness becomes even lower.

Consequences:

- if an account remains free and almost infinitely reproducible, the system continues to lose at scale;
- if protection relies only on classifiers, the operator bears the full price of secondary measures alone;
- if freeness is not tied to trust economics, the attacker gets an almost unlimited ability to stamp out weak identities.

Hence the need for a layered model where the cost of abuse is distributed not only onto sending, but onto access to a more powerful identity mode itself.

---

## 6. Target layered trust model

On the post-MVP horizon it is more reasonable to think not in binary terms—"either everything is free or mandatory bond for everyone"—but in layers.

### 6.1. Free tier

A basic account can exist with almost no economic barrier, but reduced trust applies on entry. This means:

- stricter checks and incubation tests for attachments;
- when mail is reviewed by AI agents, in ~90% of cases texts will be heavily compressed and marked with the lowest priority;
- very small daily limits; mass mailings become impossible, especially of similar content;
- limited access to business-recipient / high-trust recipient traffic;
- low trust growth rate;
- high sensitivity to negative signals.

Such an account remains sufficient for simple, honest personal activity, but is substantially less attractive for mass abuse.


### 6.2. Trusted tier

The next layer is built on time, behavior, and trust signals:

- account age;
- absence of complaints and reputation incidents;
- incoming trust-sign signals from stronger and more diverse sources;
- normal usage history;
- absence of correlation with known abuse clusters.

Here, increased capabilities are achieved not through direct payment, but through accumulation of provably normal behavior.

### 6.3. Bonded tier

The third layer introduces economic risk for the account owner:

- a bond is placed in PWM or another permitted asset;
- this unlocks higher limits;
- abuse may lead to freeze, slash mechanics, or prolonged bond lock;
- mass abuse incurs not only behavioral but direct financial cost.

This layer should not automatically be considered mandatory for all users. Its role is not to forbid free access as such, but to quickly unlock a higher trust class and make scaling abuse expensive.

### 6.4. Trust growth curve dynamics

To counter the chain reaction of weak-account self-bootstrap effectively, the layered trust model uses nonlinear trust accumulation dynamics:

- Mass horizontal interactions between Free accounts (mutual correspondence and exchange of subsidized marks) lead to minimal or practically zero growth of global trustworthiness within the provider. Such internal cycles are treated as low-weight noise and do not produce a meaningful effect.
- Significant trust growth occurs when a Free account's correspondence includes inbound messages from high-trust (Trusted/Bonded) accounts—for example, verified addresses of stores, banks, government services, and other legitimate business senders. In that case the trust curve grows substantially faster, because the probability of receiving real commercial interactions from such sources is much lower for a botnet or account factory than for real users.

Such asymmetric dynamics allow the system to naturally distinguish honest users from coordinated abuse networks without resorting to total entry barriers.

### 6.5. Trust metric propagation mechanisms

The basic circulation of trust metrics in the post-MVP model remains centralized at service operators (mail providers, messengers, telecom operators). They accumulate and evaluate account behavior metrics and pass them to the recipient in standardized form (X-PWM-Trust-Metadata headers or analogous API fields).

To confirm trustworthiness, the sender may additionally burn marks (BURN_MARK with purpose = "trust-verification"). More than 99% of such burns occur **off-chain through batching**, allowing operators to process subsidized addresses at scale without noticeable load on the blockchain. On-chain remains only a light anchor (batch-root).

The recipient remains fully passive: it is enough to verify the passed metrics and (if needed) the batch proof. Thus PWM acts not as a store of full trust histories, but as a **transmission standard** and economic anchor tied to a PWM address.

The ZK approach (selective disclosure through zero-knowledge proofs) is considered an optional advanced layer for operators who need a cryptographically provable guarantee without trusting their own key. At early stages it is not mandatory.

### 6.6. PWM address as universal trust anchor (pseudonymous blockchain passport)

A PWM address serves as the central trust binding point for all of a user's services. Each address is a pseudonymous blockchain passport to which operators of various services (email, messengers, mobile carriers, etc.) can attach accumulated trust metrics.

A user may create and warm up **multiple addresses** simultaneously, using each for separate contexts (personal correspondence, business communications, high-risk interactions). Voluntary KYC for a specific PWM address provides significant, cryptographically verifiable trust growth.

The basic trust-metric circulation scheme remains **centralized**:

- service operators independently accumulate and evaluate metrics;
- on interaction (sending mail, messages, etc.) the operator passes signed metrics tied to the sender's PWM address;
- the recipient sees a ready UX signal (stars, color coding, icons) based on these metrics.

This approach allows:

- keeping data accumulation with entities that actually see account behavior;
- making the PWM address a single, portable trust identifier across different services;
- not overloading the protocol itself with storage of full trust histories.

### 6.7. Lifecycle of subsidized accounts and tying trust to resource allocation

In the target model, subsidized accounts (Free tier) initially do not receive a personal PWM address. A newbie uses the service provider's **centralized shared address** (or that of a municipal/federal subsidizing body). All operations of such an account (including mark burns) occur from the centralized balance of that shared address, within an allocated quota.

Trustworthiness of such shared addresses **tends toward zero** and does not accumulate any metrics. They are intended solely for minimally necessary access and cannot be used for trust accumulation.

A personal PWM address is issued to the user **only after successful warm-up** (accumulation of a minimum trust level through time, normal behavior, and inbound trust-sign from Trusted/Bonded senders). From the moment a personal address is received, trust begins to grow nonlinearly (see 6.4), which opens gradual increase of allocated resources:

- higher daily mark limits;
- ability to send lead letters with burn;
- delivery priority and reduced attachment checks;
- access to business-recipient / high-trust traffic.

As long as the user does not abuse, trust growth directly correlates with expanded capabilities. At the first signs of abuse the account returns to reduced-trust mode or is fully blocked (rescue routing / emergency policy).

Thus the model explicitly separates two account types:

- **Subsidized shared addresses** — low/zero trustworthiness, strictly limited resources, no metric accumulation.
- **Personal addresses** — warm-up trustworthiness, dynamic resource increase with honest behavior.

This allows providers and authorities to subsidize basic access without mass-abuse risk, while preserving incentives to warm up and transition to a personal trust anchor.

### 6.8. General mechanics of off-chain burning exclusively from centralized balances

For subsidized accounts (Free tier), mark burning occurs exclusively from the **service provider's centralized balance** (or a municipal/federal subsidizing body). The user's personal on-chain marks are not affected.

The mechanics are fully off-chain (more than 99% of cases):

1. The user (or an application on their behalf) submits a burn request through the provider's API:
   - `POST /v1/burn-for-purpose`
   - Parameters: `purpose`, `recipient_domain` (or `recipient_address`), `amount` (within quota).
   - Signature: signature with the subsidized PWM address private key (proof of ownership).

2. The provider:
   - verifies quota and signature;
   - adds the request to a local off-chain batch;
   - accumulates thousands of such requests in a Merkle tree.

3. Once per period (for example, once per block or every few minutes) the provider publishes a single `BURN_MARK_BATCH` transaction to the blockchain (or an extended variant of the existing mechanism described in RFC 11 and RFC 14), containing the batch-root.

4. The message recipient (passively) receives from their service provider (or through a public verifier API) a Merkle proof and batch status. Verification is done exclusively through the recipient provider's trusted verifier and does not require direct access to the blockchain.

Thus PWM acts only as an **anchor** (batch-root) and economic accounting, while all mass processing remains off-chain. Detailed API specification, batch format, and verifier mechanics will be moved to a separate RFC (extension of the existing offchain-batch mechanism).

This scheme allows providers to subsidize millions of users without noticeable load on consensus, while preserving cryptographically provable burn confirmation for the recipient.

### 6.9. UX presentation of trustworthiness for the end recipient (human or AI assistant)

The goal of UX presentation is to give the recipient (human or their AI assistant) within seconds the clearest and emotionally strongest representation of the sender's trust level. The user should instantly understand whether to open a message, click links, or answer a call.

#### 6.9.1. Email and text messages

- **No trustworthiness + few/no marks (Free tier, high risk)**  
  Message text is displayed in **bright red**.  
  All links and attachments are blocked pending explicit confirmation.  
  Next to the sender — a large **skull and crossbones** icon + **10 red exclamation marks**.  
  Caption: "High fraud risk. Deletion recommended."

- **Trustworthiness present (Trusted tier)**  
  Message is marked with **4–5 green stars**.  
  Brief metrics are shown:  
  "Account exists 14 months • high probability of individual • 97% positive feedback."

- **Marks burned as lead-letter (Bonded tier)**  
  Message is marked with **4–5 gold marks**.  
  Address is automatically added to "verified contacts" (per recipient settings).  
  Icon changes to a **green shield**.

#### 6.9.2. Voice calls (including mobile)

On an incoming call, the phone screen (or AI assistant interface) shows an extended trust panel **before answer**:

- **Obvious scammer** (example: call from Romania offering to "decline a PayPal transaction")  
  Full screen — **10 large red exclamation marks**.  
  Header: **"CRITICAL FRAUD RISK"**  
  Factor list (each with a red icon):
  - Not a company / not a call center
  - Rented / virtual number
  - No inbound bond (minimum 0.1 PWM, claimable by pressing #)
  - Complaints recorded from other users
  - Geography does not match claimed activity

  Buttons: "Decline" (large red) and "Block forever". With appropriate settings, an AI assistant analyzes such a call, leaves a text comment, and automatically declines.

- **High trustworthiness**  
  Green shield + 4–5 stars.  
  Brief metrics: "Account exists 2 years • verified business • no complaints."

#### 6.9.3. Integration with AI assistant

An AI assistant (local or cloud) automatically:

- analyzes incoming mail / calls by trust metrics;
- outputs a voice or text warning ("This message is from an account with zero trustworthiness. I recommend deleting it.");
- at high trust — automatically adds the contact to verified and suggests replying.

Thus UX makes trustworthiness a **visually dominant** characteristic of any inbound interaction, turning it from an abstract metric into an intuitive safety signal.

---

## 7. Dual role of bonding

Bonding matters not only as an obstacle to abuse. It has two distinct economic meanings.

### 7.1. Obstacle to scaling abuse

Even a relatively small deposit per account creates large-scale costs for a disposable-identity factory. If the bond is subject to real loss, the attacker can no longer treat it as risk-free working capital.

This is especially important in scenarios where abuse is built on a huge number of weak accounts rather than a few expensive identities.

### 7.2. Funding secondary protective measures

An even more important role of bonding is shifting part of anti-abuse spending back to the source of the attack.

If confiscations, penalty withholdings, prolonged freezes, or other sanctions form a fund from which are financed:

- inference of AI protection models, anti-spam, and EDR;
- abuse-ops;
- reputation computation;
- manual incident review;
- additional protective measures;

then the service stops fully paying for secondary protection from its own funds.

This changes the economics of a free service itself.

The key idea here is not to make entry maximally expensive, but to:

- prevent abuse from scaling for free;
- make secondary protection partially self-compensating;
- reduce the operator's net costs of fighting abuse to near-zero values.

### 7.3. Important constraint

The bonding contour must not turn into a profit center.

If the operator begins to earn noticeably from confiscations as a standalone revenue line, a conflict of incentives arises:

- temptation to increase enforcement aggressiveness;
- higher risk of false-positive sanctions;
- service trust shifts from anti-abuse protection to punitive monetization of users.

Therefore in the target model the penalty flow should primarily fund the protective contour, not serve as a separate business model.

---

## 8. Why the chain reaction of self-bootstrap is especially dangerous

For anti-abuse economics it is not enough to assign a price to a single action. One must also prevent an abusing network from self-financing from its own activity.

This is especially important for:

- trust-sign signals;
- warming new accounts;
- subsidized lead-messaging;
- any scheme where weak accounts can collectively raise each other's usefulness.

If subsidized or cheap activity allows a large number of weak accounts to quickly elevate each other to a new trust level, a chain-reaction analog arises: the system accidentally creates not a barrier, but a self-bootstrap reactor.

### 8.1. Negative energy balance as target invariant

To prevent this, the model must have a negative economic balance for low-quality trust chains.

In practical terms this means:

- mark spend on `trust-sign` must not quickly pay back from the weak account's own natural emission;
- mark spend on warming a basic account must not by itself open a fast path to full reputation;
- lead letters toward a basic account must be so limited that their mass mutual use does not create a cheap trust factory;
- weak accounts must not be able to quickly and cheaply increase each other's value in a circle.

If for a basic account any useful trust growth requires either time, a signal from a genuinely stronger source, or significant spend of a limited resource, the chain reaction dies out naturally.

---

## 9. Principles for trust-sign semantics

`Trust-sign` must not be treated as a simple purchasable rating. Otherwise a separate gray market of warm-up and synthetic reputation appears.

In the target model trust-sign must account not only for the fact of a signal, but for its quality.

### 9.1. What should affect trust-sign weight

- source class;
- source age;
- source history of normal behavior;
- diversity of sources;
- whether the signer has its own economic risk;
- cost of error for the signal source;
- absence of correlated affiliation among signing accounts.

### 9.2. What must not be allowed

- weak accounts massively boosting each other to a high trust class;
- warm-up price lower than the price of future abuse;
- trust-sign easily sold as a commodity without risk for the signer;
- acquired trust being almost perpetual and not decaying after negative events.

---

## 10. Why this is better than a simple "everyone gets a little free marks" model

Soft subsidy is useful by itself, but it solves only part of the problem. It reduces friction for honest users well, but weakly stops an attacker who knows how to multiply identities.

The layered model gives a more complete picture:

- the honest user keeps low-friction entry;
- the attacker cannot scale infinitely for free;
- high access rights become a function not only of money, but of time, reputation, and risk;
- the operator gets a chance to partially return anti-abuse costs to the violator;
- the offchain model stops being only subsidy and becomes a system of economic risk management.

---

## 11. Boundaries between protocol and operator layer

Not everything in the described model must live inside the base PWM protocol.

It is important to distinguish two layers in advance.

### 11.1. What PWM can provide as protocol foundation

- marks as a measurable anti-abuse resource;
- on-chain burn / accounting primitives;
- a clear economic trace of actions;
- a support base for trust/reputation logic;
- ability to express limiters through policy and purpose semantics;
- support for offchain batching as a scalable operator path.

### 11.2. What likely remains on the service operator side

- specific account limits;
- exact reputation formula;
- trust-sign scoring logic;
- dispute handling;
- slashing / confiscation rules;
- recovery path for false-positive sanctions;
- combination with advertising, subscription, or other business models.

Such separation is fundamentally important. Otherwise there is a risk of overloading the protocol itself with overly specific consumer anti-abuse logic that depends on jurisdiction, segment, UX, and the business model of a particular service.

---

## 12. Regulatory and product caution

Even if this framework mainly describes anti-abuse and cost allocation, some of its branches easily enter sensitive zones.

The following require particular caution:

- mandatory monetary bond for mass retail access;
- custodial storage of user value;
- promises of yield or bonus from holding a bond;
- opaque grounds for confiscation;
- models where the service earns noticeably on penalties.

Therefore the document records a target conceptual model, not a ready product recipe for the nearest release.

---

## 13. How this model fits current use cases

### 13.0. Cultural and visual reference: geopolitical-scale deepfake threats

Popular culture already models scenarios in which deepfakes are used for destabilization at the state level. Season two of **The Capture** (2024–2025) vividly demonstrates how forged video and audio in real time can be applied to manipulate public opinion, discredit political figures, and conduct hybrid operations.

These narratives reflect a growing public demand for reliable media authentication mechanisms. Concepts of **Live Key Chip**, crypto-badges, and on-chain media anchoring (see UC-3) are in effect already "overripe": they offer a practical response to threats already recognized both in mass culture and among professionals protecting critical infrastructure and the information space.

### 13.1. UC-1 Email Anti-Spam

Here the model is most natural:

- basic correspondence can remain cheap or almost free;
- lead mailings and mass outreach get strict limits;
- business-recipient mail may require a higher trust class;
- abuse risk begins to be expressed in measurable economic form.

### 13.2. UC-2 API Protection / Anti-Automation

For API scenarios layered trust is especially useful:

- the free tier remains for low intensity;
- higher throughput opens only through trust, history, or bond;
- costs of abuse detection and inference can be partially covered by the anti-abuse contour itself.

### 13.3. General connection with offchain batching

In both cases operator offchain aggregation remains central. It not only reduces network load, but allows turning mark emission into a controlled protection budget and user-facing quotas.

---

## 14. Target invariants of the model

Below are invariants that should be preserved in any future RFCs/ADRs on this topic.

1. Free access may exist, but must not be infinitely scalable for abusers.
2. Abuse must not only be limited, but where possible partially fund secondary protection spending.
3. Trust-sign and other trust mechanisms must not allow cheap chain self-bootstrap of weak accounts.
4. A high trust class must require time, normal behavior, economic risk, or a signal from genuinely strong sources.
5. Offchain aggregation must act as an economic damper on mark emission, not as a way to hide uncontrolled inflation.
6. The penalty flow must not become a standalone profit center for the operator.
7. The base protocol must not be prematurely overloaded with overly specific consumer anti-abuse logic.

---

## 15. Communication strategy: "two pills" and the boundary between cybersecurity and social programs

The target PWM model assumes companies can offer users a conscious choice between two options:

- **Blue pill** (Free tier without economic signal) — preservation of the current model with formally free access, but with inevitable growth of risks. In the long term such an internet segment may turn into a distributed environment with minimal control, where cheap technologies (3D printing, synthetic biology) ease spread of instructions for producing controlled substances and devices. Absence of economic barriers makes effective censorship of harmful information harder and increases vulnerability to coordination of hybrid threats (color revolutions, riots, scenarios involving AGI).

- **Red pill** (Trusted/Bonded tiers with minimal economic signal) — access to verified, prioritized, and safe interaction through micro-costs or staking. This tier allows companies to deliver real cybersecurity.

The protocol does not solve social questions. Any support programs for particular categories of citizens should be implemented through direct mechanisms (for example, a targeted tax on PWM staking with subsequent distribution of marks at municipal or national level), not through forcing business into unlimited subsidy. Such a boundary allows company lawyers and lobbyists to clearly separate global cybersecurity tasks from social programs and minimize regulatory risk.

---

## 16. What this document does not assert

The document intentionally does not assert the following:

- that mandatory bonding is needed for all users;
- that a specific deposit amount is already defined;
- that slashing/confiscation semantics are ready for implementation;
- that the trust-sign formula is already designed;
- that the current MVP must include this model;
- that anti-abuse economics must be identical for email, API, messengers, and other services.

This is not a specification and not a roadmap commitment. It is a map of target direction needed so future decisions on marks, offchain scaling, operator flows, and anti-abuse use cases are not taken without a common economic framework.

---

## 17. FAQ: Model positioning and practical use of PWM

**Q: How will the main mass of PWM be stored and operated?**  
**A:** The main mass of PWM will be stored and operated in custodial services of providers of various services (B2C and B2B). Providers (mail services, messengers, carriers, corporate platforms) will hold stake and centralized mark balances, granting users quotas as part of a subsidized service.

**Q: Will personal wallets be required, and for what?**  
**A:** Personal wallets (or at least control of a private key / multisig) will be required primarily for protecting critically important infrastructure, high-trust business addresses, and cases where maximum independence is needed. For the mass user a provider custodial solution is sufficient.

**Q: Will the value of PWM coins matter for protecting information?**  
**A:** The protocol idea is not to create the high costs characteristic of many cryptocurrencies. The core concept assumes that even stake or burn worth tens of cents can effectively protect infrastructure and information worth millions. PWM value is determined not by speculative price, but by its utility as an economic barrier against abuse.

**Q: Why is the layered trust model (Free / Trusted / Bonded) better than simply subsidizing everyone with a little marks?**  
**A:** Simple subsidy reduces friction for honest users but does not stop account factories. Layered trust makes scaling abuse economically unattractive: the Free tier remains accessible but has strict limits and low trustworthiness; Trusted/Bonded tiers open only through time, normal behavior, and/or economic risk.

**Q: How will off-chain mark burning work for millions of subsidized users?**  
**A:** More than 99% of burns occur off-chain through provider batching. The user signs a request via API, the provider accumulates requests, publishes a batch-root to the blockchain once. The recipient verifies burn through their provider's trusted verifier, without direct access to the blockchain.

**Q: Can one have multiple PWM addresses and how will they differ in trustworthiness?**  
**A:** Yes, a user can create and warm up multiple addresses simultaneously. Each address is an independent pseudonymous trust anchor. Trustworthiness accumulates separately for each address, allowing different addresses for different contexts (personal, business, high-risk).

**Q: How does the protocol avoid turning into another speculative token?**  
**A:** PWM is positioned exclusively as a utility instrument. The main mass of tokens sits in provider custodial balances and is used to subsidize users, not for retail speculation. Absence of demurrage, TTL, and aggressive burn economics emphasizes utility, not scarcity.

**Q: How does the model address social fairness and "equality"?**  
**A:** The protocol addresses exclusively cybersecurity tasks and does not take on social program functions. If society considers it necessary to support particular categories of citizens, this should be done through direct mechanisms (targeted tax on staking with subsequent mark distribution), not through forcing business into unlimited subsidy.

**Q: What happens to orphaned or inactive subsidized addresses?**  
**A:** After a long period of inactivity (for example, 90–180 days) an address moves to dormant state, then may be reused by a new user with zero initial trustworthiness. This prevents accumulation of "dead" addresses and maintains trust-metric cleanliness.

**Q: How does the proposed PWM protocol differ from CBDC (central bank digital currency)?**  
**A:** PWM is a specialized utility protocol for creating economic barriers against abuse in digital communications and APIs, not a digital form of national currency.

Key differences:

- **Purpose and function**: CBDC is intended for classic monetary functions—store of value, means of payment, and settlement. PWM carries no payment function and is not intended for capital transfer or savings storage. Its sole task is economic limitation of spam, automation, and abuse through burnable marks and layered trust.

- **Emission and control**: CBDC is emitted and fully controlled by the central bank. PWM has a soft inflationary model (~5% per year) with a 21 billion genesis allocation and distribution through IPv4 claiming (ClaimIPv4Batch) and staking. Policy governance is decentralized by zones/shards with ability for local correction to meet national regulatory requirements.

- **Role in the economy**: CBDC is legal tender and may be used for any settlement. PWM exists exclusively as an internal resource of service providers (custodial balances) for subsidizing users and creating anti-abuse barriers. The main mass of PWM sits in custodial services, not with retail holders.

- **Privacy and traceability**: CBDC often assumes a high degree of traceability (programmable money). PWM is designed from the start with emphasis on minimal information disclosure: trust metrics are passed only when marks are burned and only to the recipient.

- **Scale of use**: CBDC replaces or supplements cash and bank money at the level of an entire national economy. PWM works as an infrastructure layer on top of existing communication services (mail, messengers, API, mobile) and does not claim the role of universal medium of exchange.

Thus PWM and CBDC solve fundamentally different tasks and are not competitors. PWM does not compete with the monetary system, but gives providers a tool for controlled decentralization of anti-abuse protection.

---

## 18. Short summary

The target post-MVP PWM anti-abuse model starts from the premise that free services are always subsidized by someone, and therefore their abuse costs do not disappear—they are redistributed. In this picture the protocol is useful not because it eliminates the cost of protection, but because it helps make that cost measurable, limit its scaling, and in key scenarios partially return it to the abuser.

The most promising form of this idea is not universal paywalling or mandatory bonding for everyone, but a layered trust model: a weak free layer, a stronger reputation layer, and an optional bonding layer, where offchain aggregation softens high mark emission and anti-abuse economics stops being a pure operational loss for the service.
