---
title: "Digital Gardening for Your Mind"
---

Blah blah blah...

# h1

Blah blah blah...

## h2

Blah blah blah...

### h3

Blah blah blah...

#### h4

Blah blah blah...

##### h5

Blah blah blah...

###### h6

Blah blah blah Blah blah blah...

> normal quote

- 1
- 2
- 3

1. a
2. b
3. c

> [!cite] Citation!?
> Whaaat?!

blah blah BlaH...

```mermaid
flowchart TD
    %% Title above the semi-autonomous region
    UE@{ shape: trap-t, label: "Unexpected event in unpredictable environment" }

    UE --> DMSR
    subgraph DMSR[Decision maker - system relationship]
%% Core elements
        H[Human operator]
        M[Machine]
        C[Automated]
        D[Semi-Autonomous]
        E[Autonomous]

        %% Structure

        H --> C
        H --> D
        M --> D
        M --> E

    end
    %% Class definitions
    classDef human fill:#b30000,color:white,stroke:#000
    classDef machine fill:#003366,color:white,stroke:#000
    classDef automated fill:#999999,color:white,stroke:#000
    classDef semiauto fill:#6bb36b,color:black,stroke:#000
    classDef autonomous fill:#f7c97b,color:black,stroke:#000
    classDef decison_maker fill:transparent,stroke: transparent;
    %% Apply styles
    class H human
    class M machine
    class C automated
    class D semiauto
    class E autonomous
    class DM decison_maker
    linkStyle 0 fill:none,stroke:none;
```
