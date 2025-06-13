#  Whitepaper PayWall Mark (PWM) - draft

### Author: Petrov Alexander (aka alpet) Project:

## 1. Introduction: the chaos of free and the quasi-free alternative

Today's digital world, despite its advantages, faces an exponential increase in the problems caused by the "chaos of free" mass communications. Spam, phishing, DDoS attacks and identity leaks cause enormous economic damage to corporations and governments every year, undermining user trust and reducing the overall effectiveness of digital interactions. Existing methods of combating these threats, based on centralized blacklists and reactive filtering, are insufficiently effective and always late.

PayWall Mark (PWM) offers a fundamentally new approach to solving these problems. It is a decentralized IS infrastructure designed to preempt cybercriminal activity and create an economic barrier to attack, while remaining friendly to legitimate users. We focus on creating a transparent and adaptive trust framework that integrates with existing communications platforms, offering a practical solution for corporations, governments, and individual users.


## 2. Purpose of PayWall Mark: Utility and Anti-Spam

#### The main purpose of PayWall Mark is to provide mass microtransactions centered around burning tokens with a purpose:

* Reduce unauthorized traffic: Reduce spam and phishing in email, messengers and other communications.
* Pseudonymous authorization: Use for captcha purposes, simplified passing of spam filters, special "royal" likes/thanks.
* Economic Cost of Cyberattacks: Creating a permanent, sunk economic cost for attackers when attempting mass mailings or flooding.

Unlike speculative cryptocurrencies, PWM is positioned as a utilitarian asset whose value is directly proportional to the scale of its implementation and its ability to effectively solve the identified problems.


## 3. Economic model and tokenomics

#### Base Coin (PWM):

* Issue: The base issue is 21 billion coins.
* Distribution: The primary distribution occurs through decentralized IPv4 capping on a voluntary basis. This allows the coins to be evenly distributed among potential corporate and individual users by tying them to actual Internet resources used.
* Inflation: Floating annual inflation at around 5%. Issuance performance will slow down during the summer months (least business activity) and increase from fall to spring, adapting to real business cycles. This ensures a constant replenishment of the coin pool for staking and maintains the utilitarian nature of the PWM, detaching it from speculative value.

#### Marks:

* Generation: Stamps are generated daily from each PWM coin in the staking.
* Demurrage: Each stamp has a limited lifetime (e.g., a few days). Unused stamps are "burned" or devalued after this time period. This eliminates the accumulation of stamps, prevents speculation on their value, and ensures their continued use.
* Purpose: Stamps may be exclusively burned for the benefit of the recipients of messages or services. They serve as "payment for attention" or "proof of intent" for the initiating contact.


## 4. Addressing, clustering and regional sharding

The PWM system abandons the familiar principle of a wallet address as a derivative of a public key, replacing it with the notion of a cluster address, which defines a set of addresses in the same domain

### 4.1 Cluster address format

#### **Each PWM cluster address has the following structure:**

\[**AABB**.**C...Z**\]

**AABB** - 16-bit regional code defining the domain (e.g., ISO-3166 country/state code or corporate segment for MNCs)

**C..Z** - address tail obtained from HD-tree after derivation

#### **Examples of cluster addresses:**

GB-UKD4.01d4f987ab1294c376fe

SOFT-CO.10c0f8011ae22be8ac19

DE-BE.3ff1ae20aa8312ee0089

The wallet address is formalized by the first initializing transaction, which contains metadata describing at the start at least an index and flags, both 32 bit fields. The index may refer to a ZIP code or the index of a multinational corporation with a subdivision in a segment.

#### **Examples of wallet addresses (fully initialized, including region code, index, and flags):**

GB-UKD4-PR7.0A3F9C.UsYqTRaWkLZnCoMxV

DE-BE-10176.1C92AA.YhKnZxPoLmTqWEsBVf

US-WY-98001.2B87EF.LpQrMtNaXyWeSdCvBg

#### The dot acts as a separator between the index domain and the remaining parameters, in the basic implementation a field of flags. This structure allows:

* Implement geographic/jurisdictional consensus sharding;
* encode for an individual wallet address the TTL policy, receipt behavior, acceptability of off-chain transactions, etc.

### 4.2 Principle of address generation via HD-derivation with filtering

To obtain a cluster address with the desired domain code (e.g., 0xA1B2), the HD path (m/0'/i) is searched until the first 16 bits of the cluster address match the desired value.

#### This eliminates the possibility of direct generation of "similar" addresses by an attacker and:

* increases the complexity of dust attacks by tens of thousands of times relative to standard protocols;
* makes mass creation of infrastructural fake addresses impossible;
* slows down address compromise even if the master key is leaked.

#### Expected bruteforce speed (approximately):

CPU (4 threads): ~1500 checks/sec → ~22 sec per match
GPU (massive): ~10^6 checks/sec → ~0.06 sec on average

### 4.3 Honeypots and delayed attack

#### Because of the need for bruteforcing when generating addresses even from a known master key, a natural mechanism to slow down compromise is introduced:

* There may be many HD master keys in corporate memory without explicit labeling.
* A hacker needs to go through thousands of derivations to extract the right address.
* Some addresses may be activated as traps (honeypots) and contain small amounts of PWM.

Any attempt to interact with them initiates an alarm, allowing action to be taken before real assets are compromised.

### 4.4 Cryptographic Resistance Disclamer

The signature algorithm used in PWM will be chosen from quantum stable families (e.g. CRYSTALS-Dilithium or SPHINCS+). This eliminates the possibility of tampering even if quantum computing becomes massively available. The final choice of the algorithm will be made before the release of a stable implementation of the node.

## 5. "Dumb Contracts: Simple rules for complex problems

#### "Stupid contracts" are primitive but powerful rules tied to the storage/staking address of PWM coins:

* Cold addresses: require a long spent tx confirmation - significantly more blocks.
* Corporate addresses: replenishment requires the recipient's counter-signature, without which the transaction is not complete. This acceptance completely eliminates on-chain dust attacks. A transaction published with only one signature will block the sender's funds for 24 hours, until the overlapping one with two signatures is released and debited. This allows the realization of collateral payments, where the act of accepting funds means, for example, rejecting a proposed contract and charging a fee for the fact of review.
* Conserving addresses: CLTV scheduling is prohibited until time X.
* Investment: the expense is limited to staking and is similar to a freeze in the liquidity pool. A full write-off is possible once a year or a longer period, which provides macroeconomic stability signals to the rest of the stakeholders.
* Inherited/Collegiate: requires a multi-signature or a list of trusted persons to be entrusted with the claiming of funds after a long period of address inactivity. A transaction that adds a trusted address essentially makes its signature almost equivalent to that of the original address, at least allowing spending when the specified conditions occur.
* Policy fees: an arbitrary number of policies can be docked to an address via transactions, further regulating spending and replenishment, and each such policy increases its commission costs. An excessive or closed chain of redirects can even break a payment to a commission entirely, with a cutoff of further computations.

### 5.1 Metadata binding and initialization

#### Full address activation and application of all metadata requires an on-chain transaction with type INIT that commits:

* auxiliary metadata: emergency routing policy, TTL, filtering, default behavior, and other optional parameters
* demonstrates the signature of the cluster private key owner, in addition to the sender's signature, as is common for corporate addresses

The address is considered inactive until such a transaction is published. Any attempts at replenishment transactions before the address is initialized will be rejected by the protocol. This additionally makes address spoofing scenarios more expensive.

One of the implementations of "dumb contracts" are policies that establish rules for handling deposits and spends from an address, in the format of transactions signed with the address key, and with no output spend address (only a publishing fee). Any policy can be initially active or dormant, and is activated by a user transaction. A special activation transaction records the transition of a dormant policy to an active state, and until it is published, it does not apply to the processing of transfers, but may affect the calculation of commissions.

#### The PWM protocol in the basic version will support the following:

* **Transfer Routing Policy** - Specifies that all incoming transfers are automatically routed to another address or addresses in a specified proportion. The destination addresses must be within the same L2 zone. Applicable for protection, automatic fund distribution and referral schemes.

#### **Application Examples:**

    1. Protection against key compromise - when routing is activated, funds go to a secure address.
    2. Revenue sharing between partners or departments.
    3. Automatic taxation, interest to agents and routes in referral chains.

* **TTL (Time-To-Live) policy** - automatically invalidates the address after a specified time, prohibiting any receipts or spending after that time. Can be used to permanently freeze excess liquidity of PWM coins.
* **Sender filtering policy** - allows transfers only from specific addresses or according to an approved list, can be applied in private and corporate scenarios.
* **Default Policy** - determines how unmarked anonymous transfers are handled: rejected, returned, or redirected.

Such a policy system enables high-level automation and security of financial interactions in the PWM network by configuring the behavior of the address according to the owner's business logic.


## 6. Partial centralization and the role of the arbitrator

Partial centralization is allowed to a strictly controlled extent, providing resilience to errors and catastrophic situations, especially in the early stages of network deployment. A key component is the Zone Arbitrator, elected or appointed within a particular zone, who has received delegated authority from the major coin holders. It acts as the final authority in resolving disputes or situations involving loss of access, attacks, or fraudulent activities.

#### The powers of the arbitrator:

* may initiate a temporary freeze of funds at the disputed addresses;
* manages the zone for a limited period of time (up to 30 days) by setting general policies;
* confirms reversal of transactions in case of centralized resolution of disputes;

Is required to publish reports and use multi-signature for any actions beyond the standard set of measures.
The arbitrator is not a centralized administrator - rather, he is an auditable manager whose actions are strictly limited by the protocol architecture and transparent to the community. This model provides temporary bridges between the chaos of a fully decentralized environment and the need for trust in real legal disputes.


## 7. Anti-spam and brands in practice

To effectively filter unwanted messages, the PayWall Mark protocol provides several levels of interaction with the communication infrastructure. The central place is occupied by the concept of burnable stamps, which serve as a kind of "ticket" for establishing contact. At the household level, addressable expiration is called respekt (expression of respectt), and unaddressed expiration is called annihilation. It is assumed that hyperinflationary emission of stamps by corporations (large stakeholders) will mostly be completed by addressless expiration.

#### Email:

Each message sent may contain an X-PWM header containing a hash of the stamp burn transaction or an off-chain burn reference. The recipient verifies this with a request to a decentralized or enterprise API, or an off-chain proxy, confirming that such a burn has actually occurred.

#### Messengers and APIs:

Platforms can embed the PWM protocol as a filter at the level of establishing new chats. The initiator is prompted to burn a stamp or confirm staking before allowing a dialog. This works similarly to the paid version of "request a contact".
This model results in a significant reduction in spam and phishing, while maintaining freedom of communication for bona fide participants. Unlike payment gateways, there is no transfer of value and there is no regulatory risk associated with paying for content or identity access.


## 8. Offchain Burning and L3 Scaling

While the basic logic of PWM involves burning stamps on the blockchain, off-chain scenarios open up scalability, especially when integrated with large services and infrastructure players.

#### Centralized burning via API:

A corporation or service provider that owns a significant amount of staking can act as a trusted implementer of the flaring process. The user transmits the stamp (or a digital representation of it) to the server, and the server publishes a session report of the batch burn at the end of the time interval. This reduces the load on the blockchain and lowers the cost of validation while maintaining verification capabilities.

#### Confirmation of burning:

Each user receives a digital signature confirming that their brand has been accounted for in a publicly published aggregated report (Merkle root, ID, hash, timestamp). In case of a dispute, the service is obliged to publish the user's specific position within the report.

#### Reliability and flexibility:

Within large ecosystems (e.g., corporate email servers), you can set up your own economics of burn-in and proxy aggregation without going directly to the public network.

It is allowed to transfer stamps over the API without risk of loss because each burn session is associated with an initiator address and has an internal TTL (time-to-live) policy.

#### Mass adoption and trust through public aggregation:

The architecture of the project assumes that off-chain burning will apply in more than 99% of cases, as the transactions themselves carry no monetary value. For example, social media providers will be able to publish snapshots of users' accumulated reputations with the platform's digital signature. Trust here relies on the reputation of the service, similar to likes in traditional social networks.

However, it remains possible for enthusiasts and independent validators to check the consistency of this aggregated data with real events: balances, accruals, expiration of respectts. This would require subscribing to mempool updates and keeping the full history with oneself, making the system transparent but optional for all participants.

Thus, off-chain burning makes the PWM protocol suitable for integration into centralized, high-volume services without losing the principle: "pay-as-you-go access, but not to the beneficiary, but to the trust system as a whole."


## 9. Integration with AI

With the development of generative models and the introduction of AI into mass communication processes, the problem of automated spam and abuse is taking new forms. The PWM protocol offers a natural integration point in the chain of interaction between AI agents, services and end users.

AI agents serving users (e.g., AI secretaries summarizing mail) can decide to process incoming messages only if there is a respectt attached to them. This creates an economic prioritization mechanism - AI will not waste resources on messages without an attached intention on the part of the sender.

#### API for AI integrators:

PWM can be integrated as a RESTful interface with the ability to verify stamps before performing an action. For example, to start a response generation task, a recent burned stamp must be presented on behalf of the initiator.

#### AI flood prevention protocol:

In AI-to-AI interactions, brands become a natural filtering mechanism. Only agents who have a sufficient supply of reputations or are able to burn a brand on behalf of a user will be able to initiate communication en masse. This eliminates the appearance of AI spam storms, when thousands of agents start overwhelming each other with uncontrolled requests.

In this way, PWM becomes the foundation of digital hygiene among autonomous machines as well, laying the foundations for a new etiquette of interaction among AI participants.

## 10. Business model and monetization

The PWM monetization model is extremely restrained and focused on the long-term implementation of the protocol as a public infrastructure. The priority is compatibility with grant and partnership programs rather than direct profits from asset speculation.

#### Sources of sustainable monetization:

   1.   **Integrations and technical support:** A company managing early stage PWM development can generate revenue from consulting contracts with corporations and institutions wishing to incorporate the protocol into their infrastructure. These include:

        * support for API and filter customization,

        * auditing and customization of internal PWM zones,

        * developing custom policies for dumb contracts.

   2. **Targeted grants and pilot implementations:** With the right legal and technical submission, the project could qualify for grants from major technology corporations (Google, Mozilla, Microsoft) and from digital security programs in the EU. These funds  will finance open development, testing and popularization.

   3. **Refinement of “dumb contract” functionality:** some corporate clients may require an extended fork that allows transaction metadata to be interpreted using an extended protocol compatible with the main network.

   4. **Participation in acceleration programs and public partnerships:** The PWM protocol addresses a problem that affects large public institutions. Its implementation at the EU level or as part of cyber resilience initiatives creates the basis for attracting not only venture capital funding, but also support through legal support, access to infrastructure and auditing.

   5. **Acquisition and management of an infrastructure company:** A startup company (e.g., registered in Estonia) can be structured as an integrator and assurance provider of PWM in a public environment. Once the blockchain core is placed under the management of an academic institution (e.g., MITRE or EPFL), such a company becomes analogous to Red Hat, an open protocol service business.

   6. Licenses, integrations, support

   7. Corporate Grants



## 11. Road Map

### The path to full PWM implementation requires a step-by-step, open and verifiable design. The following is an indicative plan, subject to refinement depending on funding, grants and partnerships:

#### Phase 1: Preparation of Specification and Demonstration (Q3 2025)

* Completion of a white-spec with an accurate description of the address model, TTL and inflationary parameters of stamp reproduction.
* Support for basic CLI utilities: address generation, policy visualization, test burn-in.
* Publishing technical documentation for developers and integrators.
* Launching a demo environment (svcpool.io) with tokenized stamp simulation on BNB/Solana.

#### Phase 2: Kernel Implementation (Q4 2025)

* Protocol creation in Rust: validator, console wallet, networking.

* Deploying a test network with multi-domain routing.

* Collecting feedback from the crypto community and auditing.

#### Phase 3: Ecosystem and Social Adaptation (2026)

* Realization of PoS-stacking and generation of limited validity respectts (stamps).

* Implementation of "dumb contracts"

* Full-featured multiplatform wallet with GUI

* API integrations: email, messengers, corporate gateways.

* Launch campaigns with respectts as social currency (TikTok, Discord, bloggers).

* Pilot projects in partnership with cybersecurity entities.

#### Phase 4: Institutionalization (2026-2027)

* Registering a legal entity in a suitable jurisdiction (e.g. Estonia).

* Obtaining grants, certification, participation in EU initiatives.

* Transferring control of the protocol to the supervision of an academic or industry body such as MITRE.

* Support corporate installations and authoring gateways through the project business unit.

**This roadmap is designed to phase out barriers, minimize speculation, and sustainably scale trust in digital communication.**

## 12. Long-term perspectives and infrastructure vectors of development

The adoption of PWM technology at the enterprise level, including to reduce spam and increase trust in digital communications, is naturally leading to an increase in the use of signature infrastructure and off-chain transaction mem pools. This creates the basis for wider use of digital signatures and hashed metadata in related industries.

One of the long-term threats to digital trust is the widespread use of deepfake generation technologies. In the foreseeable future, this will make it virtually impossible to distinguish falsified media content from genuine content without cryptographic proof.

The PWM protocol, with its on-chain and off-chain signature architecture, can be easily adapted for service hashing and validation of multimedia content.

### 12.1 Infrastructure of trust in media streams

#### The concept of signed broadcasting is proposed, in which:

* any recording or broadcast created by a camera (video surveillance, smartphone, professional equipment) is automatically accompanied by a frame/stream hash;

* this hash is signed by the built-in TPM (Trusted Platform Module) module of the device;

* simultaneously with the publication of media content, proof of timestamp and signatures are published in frame metadata, and in arbitrary blockchains.

#### At the user and corporate level:

* video platforms, social networks and media resources output trust indicators - presence of digital signature and key authority level;

* Officials, TV presenters in public speeches demonstrate using cryptographic badges that broadcast on a small screen QR codes of blocks of mini-blockchain (or sidechain). Each block combines hashes of past frames, the public keys of the video cameras, and GPS coordinates and time. The block is certified by an digital signature identifying the person being filmed.

* In case of security systems for critical facilities, wall screens can be used instead of badges for counter visual confirmation, which additionally excludes spoofing/freezing of the video stream even if the CCTV camera is compromised.

* there is a mutual closure of authenticity: camera key ↔ media stream signature ↔ fixed time and geographic stamp.

The data of such blocks may well be validated in L3 layer at centralized stamp burning.  This direction does not require modification of the PWM protocol itself, but fully fits into its trust meta-infrastructure architecture. In combination with the mechanisms of respectts and authorized communication, it provides high resistance of the society to manipulative information attacks.

## 13. Conclusion

The PayWall Mark project forms the foundation for a new level of trust in the digital space. Unlike attempts to monetize attention or simple access restrictions, PWM offers a decentralized economic filter infrastructure compatible with legal norms, social institutions and technical realities.

'Respects' as a form of non-monetary interaction create a bridge between Web2habits (likes, reposts) and Web3 utilitality, allowing the user to manage their engagement at any time without becoming a victim of attention or abuse.

PWM is not designed for hype and short-term profits, but for decades of operation as part of the digital ethic. Its development can proceed independently - as a public standard supported by the community and scientific organizations, or in symbiosis with large market players, for whom mitigation of harm from attacks is more important than control.

In the face of growing AI pressures and the labor crisis, PWM may become one of the pillars of adaptation - not as a currency, but as a social protocol for a new ethic of digital interaction.

The project is open to collaboration, grants, discussion and experimentation. Contact group of the project: https://t.me/svcpool_chat