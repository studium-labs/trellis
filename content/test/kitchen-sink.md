---
title: Mother of All Sinks
---

TODO...

Blah...

# Heading #1

Blah blah...

## Heading #2

Blah blah ...

### Heading 3

Blah blah blah...

#### Heading 4

Blah blah blah blah...

##### Heading 5

Blah blah blah blah blah...

###### Heading 6

Blah blah blah Blah blah blah...

A surprise mermaid diagram!!

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

Callout time!

> [!cite] Citation!?
> Whaaat?!

Listing letters and numbers in lists:

- 1
- 2
- 3

1. a
2. b
3. c

I love quotes:

> normal quote
