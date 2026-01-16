# Connector End-to-End Flow

## Purpose
Provide a step-by-step view of the tunnel lifecycle, showing which component is
responsible for each action and what data is exchanged.

## Actors
- **Datum Desktop / Device Agent**: runs on the user device, owns the iroh key.
- **Control Plane**: stores Connector + ConnectorAdvertisement, publishes DNS.
- **NSO (network-services-operator)**: programs Gateway API + Envoy policies.
- **Envoy Gateway**: translates Gateway API to xDS for Envoy.
- **Envoy + iroh sidecar**: handles inbound requests and dials connectors.

## Flow
### 1) Device starts and registers a Connector
**Responsible:** Device Agent
**Data:**
- iroh NodeID (public key)
- home relay URL
- observed addresses
- device metadata/labels

**Actions:**
- Create `Connector` (spec) for this device.
- Patch `Connector.status.connectionDetails` with NodeID + relay + addresses.

### 2) Control plane publishes discovery records
**Responsible:** Control Plane Controller
**Data:**
- `Connector.status.connectionDetails`

**Actions:**
- Publish DNS TXT records (or equivalent discovery data), e.g.
  `_iroh.<node-id>.<project>.datumconnect.net`.

### 3) Device advertises tunnels
**Responsible:** Device Agent
**Data:**
- `ConnectorAdvertisement` `spec.layer4` services (host + port)

**Actions:**
- Create or update `ConnectorAdvertisement` linked to the Connector.

### 4) Device creates HTTPProxy
**Responsible:** Device Agent (on behalf of the user)
**Data:**
- public hostname(s)
- backend endpoint URL (host + port)
- connector reference (name or selector)

**Actions:**
- Create or update `HTTPProxy`.

### 5) NSO programs Envoy
**Responsible:** NSO
**Data:**
- `HTTPProxy` backend + connector reference
- `ConnectorAdvertisement` for validation

**Actions:**
- Validate backend is allowed by ConnectorAdvertisement.
- Create Gateway + HTTPRoute + EndpointSlice (backend points to sidecar).
- Inject metadata/headers:
  - `datum-node-id`
  - `datum-target-host`
  - `datum-target-port`
  - `datum-target-proto`
- Ensure CONNECT is enabled with `BackendTrafficPolicy` (`httpUpgrade: CONNECT`).

### 6) Envoy handles inbound requests
**Responsible:** Envoy + iroh sidecar
**Actions:**
- Envoy routes traffic to sidecar (single listener).
- Sidecar reads metadata, dials by NodeID using discovery, and forwards traffic.

## Data Map (Who Writes What)
- `Connector.spec`: Device Agent (creates)
- `Connector.status.connectionDetails`: Device Agent (patches)
- `ConnectorAdvertisement.spec`: Device Agent
- `HTTPProxy.spec`: Device Agent / User / UI / API client
- Gateway API + Envoy policies: NSO
- Discovery records: Control Plane Controller

