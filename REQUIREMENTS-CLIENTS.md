# Client Requirements: Rust WebSocket Chat Server

Dit document is het contract voor externe clients die met de Rust WebSocket server communiceren (`rust-ws`).

## 1. Transport Contract

- Protocol: WebSocket (RFC 6455)
- Endpoint: `ws://<host>:<WS_PORT>/`
- Default poort: `3001`
- Dataformaat: JSON text frames
- Charset: UTF-8

Op connect stuurt de server direct een `ackName` (met een gegenereerde gastnaam) en een `system` broadcast dat de gebruiker is gejoint.

## 2. JSON Envelope

Alle berichten gebruiken een `type` veld:

```json
{ "type": "<messageType>", "...": "..." }
```

Onbekende of ongeldige JSON resulteert in:

```json
{ "type": "error", "message": "Bericht moet geldig JSON zijn." }
```

## 3. Client -> Server berichten

### 3.1 Chat versturen

```json
{ "type": "chat", "text": "Hallo allemaal" }
```

Validatie:
- `text.trim()` mag niet leeg zijn
- Max 500 characters
- Rate limiting (optioneel, via server config)

Mogelijke fouten:
- `Message cannot be empty.`
- `Message is too long (max 500 characters).`
- `Rate limit exceeded. Please wait <N> seconds.`

### 3.2 Naam wijzigen

```json
{ "type": "setName", "name": "Bas_123" }
```

Validatie:
- Lengte 2..32
- Alleen letters, cijfers, spatie, `-`, `_`

Mogelijke fouten:
- `Naam moet tussen 2 en 32 tekens zijn.`
- `Naam mag alleen letters, cijfers, spaties, - en _ bevatten.`

### 3.3 Status opvragen

```json
{ "type": "status" }
```

### 3.4 Gebruikerslijst opvragen

```json
{ "type": "listUsers" }
```

### 3.5 Ping

```json
{ "type": "ping" }
```

Met token:

```json
{ "type": "ping", "token": "abc-123" }
```

### 3.6 AI vraag (optioneel)

```json
{ "type": "ai", "prompt": "Vat TCP en UDP kort samen." }
```

Validatie:
- AI moet enabled zijn op server
- `prompt.trim()` mag niet leeg zijn
- Max 1000 characters
- AI rate limit per user

Mogelijke fouten:
- `AI is niet geactiveerd op deze server.`
- `Geef een vraag op. Gebruik: /ai <vraag>`
- `Vraag is te lang (max 1000 tekens).`
- `Rate limit bereikt (max <N>/min). Probeer over <S> seconden.`
- `AI request timed out after <N> seconds.`
- `AI service tijdelijk niet beschikbaar.`
- `AI service error: <HTTP_STATUS>`
- `Kon AI antwoord niet verwerken.`

## 4. Server -> Client berichten

`at` is een Unix timestamp in milliseconden (u128 op server).

### 4.1 `ackName`

Wordt gestuurd bij connect en na succesvolle rename.

```json
{ "type": "ackName", "name": "guest-a1b2c3", "at": 1733312400000 }
```

### 4.2 `system`

Join/leave/rename events:

```json
{ "type": "system", "text": "guest-a1b2c3 heeft de chat betreden.", "at": 1733312400001 }
```

### 4.3 `chat`

```json
{
  "type": "chat",
  "from": "Bas",
  "text": "Hallo allemaal",
  "at": 1733312410000
}
```

### 4.4 `status`

```json
{
  "type": "status",
  "version": "0.1.0",
  "rustVersion": "1.82.0",
  "os": "macos",
  "cpuCores": 10,
  "uptimeSeconds": 42,
  "userCount": 3,
  "peakUsers": 8,
  "connectionsTotal": 15,
  "messagesSent": 112,
  "messagesPerSecond": 2.67,
  "memoryMb": 18.34,
  "aiEnabled": true,
  "aiModel": "openai/gpt-4o"
}
```

`aiModel` ontbreekt als `aiEnabled=false`.

### 4.5 `listUsers`

```json
{
  "type": "listUsers",
  "users": [
    { "id": "8b7e27d4-6f2f-4cd7-a939-0a44a3f90b2e", "name": "Bas", "ip": "192.168.1.10" },
    { "id": "b2209c7e-60f2-466f-952f-6ea2360e94ab", "name": "Eva", "ip": "192.168.1.11" }
  ]
}
```

### 4.6 `pong`

Zonder token:

```json
{ "type": "pong", "token": null, "at": 1733312420000 }
```

Met token echo:

```json
{ "type": "pong", "token": "abc-123", "at": 1733312420050 }
```

### 4.7 `ai`

AI antwoord wordt naar alle connected users gebroadcast.

```json
{
  "type": "ai",
  "from": "Bas",
  "prompt": "Vat TCP en UDP kort samen.",
  "response": "TCP is betrouwbaar en connection-oriented; UDP is sneller en connectionless.",
  "responseMs": 842,
  "tokens": 121,
  "cost": 0.00042,
  "at": 1733312430000
}
```

`tokens` en `cost` kunnen ontbreken.

### 4.8 `error`

```json
{ "type": "error", "message": "Message cannot be empty." }
```

## 5. Verwachte Client Flow

1. Open WebSocket connectie naar server.
2. Verwerk initiële `ackName`.
3. Verwerk `system`/`chat` broadcasts asynchroon.
4. Stuur commando's (`status`, `listUsers`, `ping`, `setName`, `chat`, optioneel `ai`).
5. Render `error` altijd zichtbaar voor gebruiker.

## 6. Compatibility Notes

- Dit contract beschrijft de Rust backend (`rust-ws`) en is leidend.
- De Bun backend (`ws-server.ts`) is deprecated en kan afwijken in velden.
- Client projecten moeten defensief parsen: onbekende velden negeren, ontbrekende optionele velden toestaan.
