# Project Requirements 

## Doel en Scope
Dit project is een chat server / client op basis van websockets.

Er moeten twee entryponis zijn (processen)
- een webserver op de client code naar de browser te serveren
- een websocket backend om de chats af te handelen

Beide mogen in een project zitten maar moeten afzonderlijk van elkaar gestart kunnen worden voor
als we laster de twee verandwoordelijkheden over verschillende servers / containers gaan onderbrengen.

## Doelgroep en Use Cases
- Primaire gebruikers: Bezoekers van chat server moeten met elkaar kunnen chatten
- Belangrijkste use cases / user stories (bulletlijst):
* een bezoeker opent de site in zin browser en laad de (JS) frontend
* hier kan de gebruiker een nick name instellen en direct alle live chat voorbij zien komen
* Er is een chatwindow en onderin een plek waar de bezoeker een bericht kan invoeren / verzenden
* met een slash (/) kunnen "systeem" commando's gegeven worden, zoals: naam veranderen, status opvragen (en later wellicht nog andere zaken)

## Functionele Eisen
- Routes/endpoints en gedrag:
-- frontend: / (home, chatwindow), /status server status
- Invoer/uitvoer validaties: Basis validatie
- Edge cases en foutafhandeling: model voor gebruikers als er iets is misgegaan (bv verbinding met de backend) met de melding
- De WebSocket backend is geïmplementeerd in Rust (Axum/Tokio) — zie `rust-ws/`
- Er is ook een Bun/TypeScript versie beschikbaar voor testen (deprecated)

## Niet-functionele Eisen
- Performance (latency/throughput): voor nu geen eisen
- Beschikbaarheid/uptime: voor nu geen eisen
- Schaalbaarheid: websocket en webserver processen scheiden
- Beveiliging (authN/Z, rate limiting, input sanitatie): In deze fase geen beveiliging of banning
- Observability (logging/metrics/tracing): Het server process moet logging: 
* nieuwe gebruiker
* gebruiker X stuurt bericht
* gebruiker X gaat weg
- er moet een archument zijn om te bepalen waar er naartoe gelogd worden (stdout, file)


## API & Data
- API contract (request/response modellen, statuscodes):
Wat betrefd de communicatie tussen fronend en backend (via websocket)

### client naar server
* verander gebruikersnaam
* stuur bericht
* vraag status op
* vraag lijst gebruikers op

### server naar client
* bevestig verandering gebruikersnaam
* berichten van andere gebruikers
* antwoord status (server status)
* antwoord lijst gebruikers

- Data modellering/opslag (indien van toepassing):

## Configuratie en Omgevingen
- Omgevingsvariabelen:
- Omgevingen (dev/stage/prod) en verschillen:

## Kwaliteit & Testing
- Teststrategie (unit/integration/e2e): voor nu geen
- Coverage/doelstellingen: voor nu geen
- Testdata en fixtures: voor nu geen

## Deploy & Operations
- Deployproces en rollback-strategie: voor nu geen
- Monitoring/alerts: voor nu geen
- Backups en herstel: voor nu geen

## Open Vragen / Assumpties
- let op dat de afspraken in AGENTS.md gehonoreerd worden
