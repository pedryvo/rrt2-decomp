# Multiplayer (rede e regras com vários humanos)

Objetivo: o que o `RT2_PLAT.EXE` faz quando há mais de um humano, para que o multiplayer do clone (lockstep determinístico entre cópias do clone, sem falar DirectPlay com o original) tenha **as mesmas regras de jogo**. Tudo vem do executável; `docs/reference/manual-rules.md`, o manual e `readme3.txt` só servem de pista, e as divergências estão na seção 7.

Convenções: `[V]` visto no código com leitura direta e consistente; `[S]` inferido do código sem teste dinâmico; `[?]` incerto (seção 8). Endereços são VA do EXE (base `0x400000`). "Rede" quer dizer `net_player_count() >= 2` (`0x46ae50`, que devolve `max(1, DAT_00577dc0)`), a condição que o código testa em todo lugar; `DAT_00573f1c` (`GameIsMultiPlayer` dos eventos) só é gravada uma vez, ao iniciar o mundo (`0x479b20`: `1 < net_player_count()`).

## 0. Resumo

1. **Arquitetura.** É **lockstep determinístico sequenciado pelo anfitrião**: só comandos trafegam; cada máquina roda a simulação inteira. Um cliente manda o comando ao anfitrião; o anfitrião o enfileira, junta os comandos num **lote** carimbado com um **minuto de jogo** e com um **horizonte de tempo** (até onde cada máquina pode avançar) e transmite o lote a todos (inclusive a si mesmo, pelo mesmo caminho). Todas as máquinas executam os comandos no mesmo minuto de jogo. O estado **não** é trocado; só o mapa/save inicial (compactado) e um checksum de conferência. O DirectPlay serve apenas de portador de datagramas e de gestão de sessão (a confiabilidade, a fragmentação e o CRC são do próprio jogo). O anfitrião não é autoritativo sobre o estado, só sobre a ordem e o ritmo (seção 1).
2. **Dessincronização.** Existe um checksum de 26 bytes (RNG, caixa das empresas, caixa das pessoas, trens, indústrias) trocado todo mês (dia 1) e comparado, mas a **reação** (mensagem "Out of Synch", novo `cmd_seed_rng` aleatório e **pausa**) só liga com o flag de depuração "Network Debug Mode" (campo `+0x2c` do bloco de sessão apontado por `0x57b400`, desligado por padrão). Sem ele a divergência passa em silêncio (seção 1.8). Nunca há reenvio de estado.
3. **Pausa e diálogos.** Em rede **o tempo nunca para por causa de diálogo** (a opção "Keep game going in dialogs" é sempre verdadeira); os diálogos e jornais passam a **fechar sozinhos** (5, 6 ou 8 s), salvo votos. Velocidade e pausa são o comando `0x0b`, **global**: qualquer humano muda a velocidade de todos, sem votação. "Pausa" (velocidade 0) em rede não congela o relógio: ele anda no máximo 1 minuto de jogo por segundo real (seção 3.1).
4. **Desconexão.** Quem sai manda a mensagem `0x0c`; quem some é cortado depois de 90 s sem notícia (23 s sem ouvir gera uma sonda). O anfitrião que cai é substituído pelo jogador de menor id (migração). A remoção vira o comando `0x01` (sincronizado) e **as pessoas humanas do jogador passam a IA** (aviso LNG 2319). Não existe entrar no meio da partida nem reconectar: a sessão é fechada ao lançar o jogo. Se sobrar um humano, o jogo continua como partida local (todos os `net_player_count() < 2` voltam a valer).
5. **Regras de simulação diferentes em rede** (o manual diz "idêntico ao single player", o código não): os trens andam em passos de um dia (`trains_advance(1440)`), o RNG é re-semeado todo dia, o jornal e o relatório anual não abrem, cheats, editor, recarregar mapa e salvar/carregar pelo jogo ficam desativados ou limitados, e a falência não remove trilho (seções 3 e 5).

## 1. Arquitetura de rede

### 1.1 Camadas (de baixo para cima)

| camada | endereços | papel |
|---|---|---|
| DirectPlay 4 (COM, `dplayx.dll`) | `0x44ec10` init, `0x44eba0` provedor disponível, `0x44eeb0` fecha, `0x44ef60` escolhe provedor, `0x44f3e0` enumera sessões, `0x44f730` cria sessão, `0x44fac0` entra, `0x44fa90` fecha, `0x44fda0` envia, `0x44fe10` recebe, `0x44ffb0` registra o anfitrião da sessão em que se entrou (nome "DPServer") | só datagramas e sessões; `Send` com flags 0 (**não** garantido): a confiabilidade é do jogo [V] |
| transporte do jogo | `0x4a0410` envia mensagem tipada, `0x49fe80` envia fragmento, `0x4a0320` registra reenvio, `0x4a1d80` recebe pacote, `0x4a0540` despacha a mensagem, `0x4a10e0`/`0x4a1430`/`0x4a1050` temporizadores, `0x4a1210`/`0x49f990` ack parcial | fragmenta, CRC, confirma, reenvia (seção 1.2); **livre no clone** |
| jogadores e sessão | `0x49c9a0` inicia, `0x49cac0` entra, `0x49cc50` cria, `0x49ce50` batimento, `0x49d240` adiciona jogador, `0x49d6f0`/`0x49db90` saída e migração | tabelas de jogadores e anfitrião (1.4) |
| comandos em rede | `0x4af6d0` `cmd_send`, `0x4af500` anfitrião recebe, `0x4af8c0` `cmd_net_flush`, `0x4afc80`/`0x4afd50` recebe lote, `0x4af3d0` `cmd_queue_run`, `0x4a5cb0` `sim_advance` | lockstep (1.5) |
| lobby | `0x4b75b0` tratador de `setup.win`, `0x4b51e0` páginas, `0x4b69b0` `net_game_start` | seção 2 |

O jogo vê **quatro provedores** (mesmo código, só muda o GUID de serviço do DirectPlay): `2` TCP/IP, `3` IPX, `4` Modem, `5` Serial (`0x4c6850`, `0x475470`, `0x48c950`, `0x4b1330` chamam `0x44eba0(n)` e `0x44ef60(n, texto)`; erros LNG 457, 372, 382, 383). `DAT_00577db8` guarda o provedor ativo (0 = rede inativa) e `DAT_00578b08` é "rede ativa". GUID da aplicação (sessões e jogadores): `{84E7A1A0-D548-11D2-8757-00104BCAB3FB}` (`0x535ab0`). Sessão criada com `DPSESSION_MIGRATEHOST` (flags 4) e **32 jogadores no máximo** (`dwMaxPlayers = 0x20`, `0x44f730`) [V]. O nome padrão do servidor é `RT2_Host` (`0x55b078`); o nome real vem do campo `+0x19c` das opções do jogo (LNG 517 "What do you want to call your server?"). O código de sessão existe em **quatro cópias idênticas por provedor** (enumerar `0x4c68d0`/`0x4754f0`/`0x48c9d0`/`0x4b13b0`, criar `0x4c68e0`/`0x475500`/`0x48c9e0`/`0x4b13c0`, fechar a sessão a novos jogadores `0x4c6930`/`0x475550`/`0x48ca30`/`0x4b1410`, enviar `0x4c6910`/`0x475530`/`0x48ca10`/`0x4b13f0`) mais a base `0x44f3e0`/`0x44f730`/`0x44f600`/`0x44fda0`; o despachante (`0x49c8b0`, `0x49cc50`, `0x49cdb0`, `0x4a0410`) escolhe a cópia por `DAT_00577db8`. A linha de comando `/A<dígito>` (`0x465050`, `DAT_00574b70`) lança o jogo **a partir de um lobby DirectPlay** (`IDirectPlayLobby`, `0x450080`): entra direto numa sessão que o lobby escolheu, sem as telas K1/K2 (fora do escopo do clone, seção 8). O botão "Gathering Net" (LNG 1694/1695, serviço de pareamento da Gathering of Developers) não tem código de rede: o botão `0xc5d` só abre um endereço da web.

### 1.2 Transporte do jogo (livre no clone)

Tudo abaixo pode ser substituído por TCP/QUIC; fica registrado porque define os **tipos de mensagem** (1.3), que são regras.

- **Pacote** (`0x49fe80`): `+0` u8 tamanho do payload (máximo `0xf8` = 248, `DAT_00578dec`), `+1` u16 CRC (`0x4ef4d0`) calculado com o campo zerado, `+3` u16 id da mensagem, `+5` u16 índice do fragmento, payload a partir de `+7`. No fragmento 0 o payload começa com `tipo u8` e `nº de fragmentos u16` (`0x4a0410`), depois o corpo.
- **Confiabilidade por tipo** (`0x55a9ac`, um byte por tipo): confiáveis só `7`, `8`, `9`, `0x10`, `0x11`, `0x12`; todos os outros são "dispare e esqueça". Mensagem confiável: guardada em `0x578e40`; reenviada a cada 3 s (fragmento único) ou, com vários fragmentos, pergunta o estado (tipo `3`) e o par responde com um mapa de bits (tipo `2`) ou com o ack final (tipo `1`); descarta depois de 30 min. Receptor: mensagens completas ficam 120 s numa lista de duplicatas (`0x578e44`); montagens incompletas expiram em 30 min (`0x4a1430`).
- **Destino** `0x7fffffcf` = todos os outros jogadores (pseudo-id); `0x4a0320` o expande em um envio por jogador do roster.
- Recebimento: `0x44fe10` (um `Receive` por chamada, descarta o que veio de si mesmo) é chamado de dentro de `clock_ms` (`0x4efea0`) e do relógio da interface (`0x402635`): a rede é sondada toda vez que o jogo lê o relógio [S].
- Opções de depuração (menu "Debug" do recurso `MNUDFLT`, desligadas): "Simulate Extra Lag" (`opts+0x230`, tabela de vazão `0x55aa4c` = 999999, 14375, 5625, 2750, 1500 B/s), "Simulate Extra Packet Loss" (`opts+0x220`, "Intentional packet hose": corrompe ou trunca pacotes; tabela de probabilidades 0, 0,1, 1, 5, 10, 25, 50 %), "Show multiplayer statistics" (`opts+0x4c`: contadores Messages/Packets Sent/Rcvd, Bytes, Bad Packets, BPS, `0x4a1590`), "Network Debug Mode" (`[0x57b400]+0x2c`) e "Network Ignore Heartbeats" (`[0x57b400]+0x28`). O bloco de sessão (`0x57b400` é um ponteiro; `malloc(0x48)` em `net_session_init` `0x4efd50`, que o zera) tem ainda `+0x3c` = identidade persistente do jogador (5.3); as duas opções de depuração só se ligam pelo menu "Debug" (`0x4658f0`, ids `0xa42a`/`0xa42b`).

### 1.3 Mensagens (`0x4a0540`, byte de tipo)

| tipo | rel. | conteúdo | efeito no receptor |
|---|---|---|---|
| `1` | não | id da mensagem u16 | ack: apaga a mensagem da lista de reenvio |
| `2` | não | id u16, nº u16, mapa de bits | ack parcial: reenvia os fragmentos que faltam (`0x49f990`) |
| `3` | não | id u16 | consulta de estado: responde `1` ou `2` (`0x4a1210`) |
| `4` | não | texto C (até 900 caracteres) | chat: `0x49cd00` (na tela do lobby `0x4b66f0`, no jogo `0x46c0b0`) |
| `5` | não | registro de jogador (`0x147` bytes, 1.4) | só **antes do início** (`0x578b00 == 0`) e só o anfitrião age: adiciona o jogador ordenado por id e devolve `6`; depois reenvia a configuração (comando de lobby `5`, `0x4b9f80`) |
| `6` | não | `[+0]` nº de sequência do lote (`0x57921c`), registro DirectPlay do anfitrião (`0x4f` bytes), id do anfitrião, nº de outros, registros `0x147` de todos os outros | resposta ao `5`: o convidado adota a lista (`0x49d240` por jogador), lembra o nº de sequência (`0x4afba0`) e acorda o `Join` (`DAT_00577dd0 = 1`) |
| `7` | sim | pacote de comando (`id u8, tamanho u32, ..., corpo`) | cliente para o anfitrião; o anfitrião acrescenta à lista de saída (`0x4af500`, 1.5) |
| `8` | sim | lote do anfitrião (1.5) | `0x4afc80`/`0x4afd50`: aplica em ordem de sequência |
| `9` | sim | id de rede do novo anfitrião u32 | migração: `0x49db90` |
| `0xa` / `0xb` | não | vazio | ping e resposta (lobby, `0x49d680`): `ping = agora - ultimo_envio` em ms, grava `registro+0x10` e redesenha a lista de jogadores (`0xc90`) |
| `0xc` | não | vazio (id do remetente é o do pacote) | o remetente está saindo (`0x49c770` envia; `0x49d6f0` trata) |
| `0xd` | não | vazio | batimento: só atualiza "último ouvido" |
| `0xe` | não | vazio | sonda "você está aí?": responde `0xd` |
| `0xf` | não | `u32 minuto + registro de checksum de 26 bytes` (tamanho `0x1e`) | `0x49de20` (1.8) |
| `0x10` | sim | `u32 tamanho_descompactado, u32 é_jogo_salvo, u32 CRC, dados compactados` | arquivo de mapa ou jogo salvo (1.9); responde `0x12` |
| `0x11` | sim | vazio | "estou carregado" (`bit 3` do `+0x2a` do remetente), usado na barreira (1.7) |
| `0x12` | sim | vazio | "recebi o arquivo" (`bit 2` do `+0x2a`) |
| `0x13` | não | vazio | pergunta "já carregou?"; se já, responde `0x11` |

### 1.4 Sessão, anfitrião e tabelas de jogadores

- **Duas tabelas.** Sessões DirectPlay (`0x577e2b`, n em `0x577dbc`, registros de `0x4f` bytes: `+0` id do jogador DirectPlay, `+4` id de rede do dono, `+0x2c` nº de jogadores, `+0x30` nome) e o **roster** de jogadores do jogo (`0x577e2f`, n em `0x577dc0`, registros de `0x147` bytes), **ordenado por id de rede crescente** (`0x49d240` insere antes do primeiro id maior). O roster é o que a simulação enxerga: `net_player_count()`.
- **Registro do roster (`0x147`)** [S]: `+0` id de rede u32 (aleatório de 31 bits: `(rand%32000+1)<<16 + rand%32000+1`, `0x4c68e0`; o id do convidado vem de `0x44fac0`); `+4` id do jogador DirectPlay (casa o `from` de `0x44fe10` com o id de rede); `+8` u32 versão mínima de protocolo (`0x2329` = 9001); `+0xc` u32 **versão** (`0x2334` = 9012; `net_min_peer_version` `0x49d920` é o menor entre os jogadores, 9012 sem rede); `+0x10` ping em ms; `+0x14` hora do último ping enviado; `+0x18` última vez que se ouviu esse jogador; `+0x1c` última vez que se enviou algo a ele; `+0x20` última sonda; `+0x24` u32 **identidade persistente do jogador** (`[0x57b400]+0x3c`, id aleatório gerado por `net_session_init` `0x4efd50`, guardado nos saves: 5.3); `+0x28` u16 memória física em MB (`0x4ef520`; coluna "MB" da tela, LNG 2219); `+0x2a` u8 **flags**; `+0x2b` o **perfil do jogador** (284 bytes copiados de `opções+0x80`): nome (`+0x2b`, 30 caracteres, é o nome em "Player %1 has exited"), retrato u16 em `+0x4a` (`perfil+0x1f`) e descrição de 250 caracteres em `+0x4c` (`perfil+0x21`). O diálogo H3 (botão "Change", `0x4ac510`) edita esse perfil; LNG 2912.
- **Flags `+0x2a`**: bit 0 **aprovação** ("luz" ao lado do nome); bit 1 "mapa/jogo verificado" (resposta ao comando de lobby `9`); bit 2 "tem o arquivo" (o anfitrião nasce com 6); bit 3 "carregado" (barreira); bit 4 "arquivo já enviado" (anfitrião, `0x4b3640`).
- **Anfitrião.** O criador da sessão (`0x4c68e0`, que sorteia dois ids: o da sessão e o de rede) fica com `DAT_00577dc8 = DAT_00577dcc = id da sessão` (`net_is_host` = `DAT_00577dc8 != 0`) e `DAT_00577dc4 = id de rede local`. Convidado: `0x49cac0` entra na sessão, manda o próprio registro (tipo `5`) ao anfitrião até **6 vezes com espera de 5 s** e desiste com LNG 381 "Unable to establish connection". O anfitrião lógico é o dono do registro em `sessões[host].+4`; **migração**: quem tem o menor id de rede depois do anfitrião que sumiu vira o novo anfitrião (`0x49d6f0`/`0x49ce50`: percorre o roster, pega o primeiro id diferente do que caiu, manda `9`, `0x49db90` ajusta `+4`, `DAT_00577dc8 = DAT_00577dcc` se for eu, `DAT_00579230 = 1`).

### 1.5 Lockstep de comandos

**Envio** (`cmd_send` `0x4af6d0`, `commands.md` 1.1): em rede, o cliente manda o pacote (`id u32, tamanho u32, +8 tempo, corpo em +0xc`) ao anfitrião com a mensagem `7` (`0x4a0410(7, id_do_anfitrião, P, P.tamanho)`) e marca `DAT_00579224 = 1`; o anfitrião acrescenta o pacote à lista de saída `0x574c94` e chama `cmd_net_flush`. Ninguém executa o comando antes de o lote voltar.

**Lote** (`cmd_net_flush` `0x4af8c0`, só o anfitrião, com `0x574c18 != 0` (mundo pronto) e `0x574c04 == 0` (não carregando); no lobby também): sai quando a lista de saída não está vazia **ou** passou 1 s desde o último lote (lote vazio de batimento). Formato (buffer de 5000 bytes, cresce de 5000 em 5000):

```
+0   u32  seq     = ++0x57921c                     ; nº de sequência do lote
+4   u32  stamp   = H_anterior (0x579214)          ; minuto em que TODOS os comandos do lote valem
+8   u32  horizonte H_novo
+0xc registros: { id u8, tam u16 = bytes do corpo + 3, corpo (pacote a partir de +0xc) }...
H_novo = max(H_anterior, T + minutos_de_jogo(2000 ms na velocidade atual) + 1)      ; T = game_time_minutes do anfitrião
         se o anfitrião acabou de assumir (0x579230): + minutos(8000 ms), e nunca menos que (0x579220 ou T) + 1
mundo ainda não pronto (0x574c18 == 0): seq normal, stamp = horizonte = 0
```

O anfitrião manda o lote a todos (`0x4a0410(8, todos, ...)`, confiável) e o **processa localmente pelo mesmo caminho** (`0x4a0540(8, ...)`), então o anfitrião também só executa o que o lote diz. Resumo da fórmula: o horizonte é "o relógio do anfitrião mais 2 s reais de jogo"; o carimbo de um lote é o horizonte do lote anterior, que nenhuma máquina pode ter ultrapassado.

**Recepção** (`0x4afc80` → `0x4afd50`): os lotes têm de ser aplicados **em ordem de `seq`**: um lote só é aplicado se `seq == 0` ou `seq - 1` está no anel dos últimos 100 aplicados (`0x579084`); senão espera em `0x574c90` e é retomado quando o antecessor chega. Ao aplicar: lembra `seq` no anel, apaga `seq - 1`, grava `DAT_00579220 = horizonte` (se != 0; se `DAT_00579224` estava marcado, zera e leva `DAT_00579080 = max(.., horizonte)`), e cada registro vira um pacote `(id, tam + 9, +8 = stamp, corpo)` enfileirado em `cmd_queue` (`0x574c8c`). Exceção: **`0x0b` (velocidade/pausa) com o mundo montado (`DAT_00578b04 != 0`) executa na hora da chegada**, não no minuto carimbado [V].

**Execução**: `sim_advance` (`0x4a5cb0`, `time.md` 6) limita o avanço ao horizonte (`T + dt > H` => `dt = H - T`; sem horizonte recebido `H = T` e nada avança), e a cada passo executa de `cmd_queue_next_time` (`+8` do primeiro pacote) em diante: avança até o minuto do comando, roda `cmd_queue_run` (`cmd_execute` dos pacotes com `+8 <= T`), continua. Como o carimbo é sempre o horizonte anterior e nenhuma máquina o passou, **todos executam cada lote no mesmo minuto de jogo, na mesma ordem**. A ordem é a de chegada ao anfitrião; não há validação do remetente (qualquer cliente pode mandar comando de qualquer empresa: confiança total; ver seção 6).

**Ritmo do quadro** (`game_frame_update` `0x405540`, `time.md` 5): `dt0 = minutos(ms)`; em rede `folga = H + (dt0 - T)`; se `folga > 0`: `seg = minutos(1000 ms)`; se `seg <= 0` (velocidade 0) ou `folga/seg > 10.0` (f32): `dt = H - T` (alcança o horizonte de uma vez); senão se `folga/seg > 3.0`: `dt = ftol(razão * dt0 * (1/3))` (acelera até ~3,3x); senão `dt = dt0`. Depois: se `T + dt < 0x579080` (o jogador mandou comando e o lote ainda não chegou) e `H != 0` e `dt <= 0x579080`: `dt = 0x579080` (valor absoluto usado como delta, depois cortado pelo horizonte: na prática **corre até o horizonte**), e por fim `T + dt <= H` sempre.

### 1.6 Sessão sincronizada e passos que só existem em rede

- Em rede o RNG é **re-semeado todo dia** (`sim_advance`, `rng.md`): `rng_seed(ftol((f64)(T + game_seed) + pessoa[0].caixa(+0x12a)))`, e `net_game_start` semeia com `game_seed` (`0x57486e`) no início (`DAT_00578b00 = 1`).
- Os trens andam **uma vez por dia**, `trains_advance(1440)` (`0x4a58f0`), precedido de `0x46b150` e seguido de `0x46b290` (suspende e retoma a pré-visualização de construção); em jogo local cada trem anda com o `dt` de cada quadro (`trains.md`, `time.md`). O resultado de movimento em rede é, portanto, o de passos diários de 1440 min, **independente da taxa de quadros**.
- O resto (economia, indústrias, IA) usa o mesmo código; só mudam as condições da seção 3.

### 1.7 Início sincronizado

Ver 2.6. Em resumo: o anfitrião manda o comando de lobby `6`; todos mostram "Launching Game..." (LNG 2211), o anfitrião envia o mapa/save a quem não o tem, todos esperam o arquivo, a sessão é fechada para novos jogadores, o RNG é semeado com `game_seed`, o mapa é carregado (`0x479b20` ou `0x479710`) e uma **barreira** (`0x4b34c0`, mensagens `0x11`/`0x13`, LNG 2692 "Waiting for other players...") só libera depois que todos terminaram de carregar.

### 1.8 Dessincronização

`net_time_sync` (`0x49e0b0`, chamada no fim de cada dia por `sim_advance`): se a rede está ativa e (`Network Debug Mode` ligado **ou** dia do mês == 1) monta o registro `0x46c680` e manda `0xf` **só ao anfitrião** (o anfitrião chama `0x49de20` direto para si; os clientes mandam o seu ao anfitrião); logo só o anfitrião compara. Registro de **26 bytes** (`net_sync_checksum_build`):

| offset | tipo | conteúdo |
|---|---|---|
| `+0` | u16 | `rng_get_state() & 0xffff` |
| `+2` | f64 | empresas (conjunto `0x57072d`): para cada uma `s += q(0)`; `s += q(0xd) * 0x535420`; `s = s * 0x537588 + q(0xe) * 0x537598` (`q` = `0x43a660`/`0x43a060`, métricas da empresa) |
| `+0xa` | f64 | pessoas (conjunto `0x570779`): `s = (s + pessoa.+0x12a) * 0x537588` |
| `+0x12` | u32 | soma sobre trens de `trem.+0x69 * 3` mais, se `trem.+0x9c != 0`, `modelo.+0x17` |
| `+0x16` | u32 | soma sobre indústrias (lista `0x574c74`) não removidas: `+0x1c*7 + +0x1a*3 + byte(+6)*0xb + u16(+0) + u16(+8)` mais, se `+8 != 0`, `Σ byte[i] * i` de 44 bytes do registro de carga (`0x574c60`) |

Receptor (`0x49de20`, só o anfitrião na prática, com mais de 1 jogador): guarda o registro numa tabela de 100 posições de 32 bytes (`0x577e38`: `valido u8, indice_do_remetente u8, minuto u32, registro 26`), substitui a posição de menor minuto, e compara com todo registro guardado **do mesmo minuto** e **diferente**: marca bits 0 (RNG, só com Network Debug Mode), 1 (empresas), 2 (pessoas), 3 (trens), 4 (indústrias). **Reação** (somente com `Network Debug Mode`): mensagem na tela por 5 s `"Out of Synch (flags)"` (LNG 2320, `0x502d60`), limpa a tabela, e o anfitrião (único que compara) envia `cmd 0x2b` (`cmd_seed_rng`, semente `crt_rand_range(1, 100000)`) e `cmd 0x0b` com `+0xc = -velocidade` (**pausa**, velocidade 0). Os dois comandos vão pelo lote a todas as máquinas. Nada corrige o estado: só realinha o RNG e para o relógio (o `cmd 0x0b` leva `-velocidade`, ou seja, pausa; como em rede a pausa anda 1 min/s, 3.1, nem o relógio pára por completo). Sem a depuração: nenhuma reação.

### 1.9 Transferência do mapa/jogo (`0x4b3640`, só o anfitrião, só com 2+ jogadores)

Se algum jogador não tem o arquivo (flag bit 1 sem bit 2): o anfitrião lê o arquivo (`maps\<nome>.MP2`, depois `.MAP`; jogos: `games\<nome>.GM2`, depois `.GAM`, também sob a pasta da instalação `DAT_00574a30`), compacta (`0x4feba0`, LNG 2695 "Compressing map data..."), monta `{u32 tamanho_original, u32 é_jogo, u32 CRC, dados}` e manda o tipo `0x10` a cada jogador sem arquivo (LNG 2696 "Sending map data..."), repetindo até todos terem o bit 2. O receptor (`0x4a0540` caso `0x10`) confere o CRC (erro LNG 2865 "Unrecoverable data transmission error"), descompacta (LNG 2706 "Decompressing map data..."), grava o arquivo (`.MP2`/`.GM2` se existir um com esse nome, senão `.MAP`/`.GAM`) e responde `0x12`. A tela mostra "Receiving Map Data %1" (LNG 2707). Quem não tem o arquivo e é convidado espera em `net_game_start` com LNG 2693 "Waiting to receive map data...".

## 2. Opções da partida multiplayer (lobby)

### 2.1 Fluxo de telas

`setup.win` (a mesma janela de 1024x768 do menu, `screens.md`) é conduzida por `0x4b75b0` (tratador de eventos) e `0x4b51e0(página)`. As páginas relevantes (a numeração é do código; os nomes são interpretação [S]):

| página | tela |
|---|---|
| `1` | menu principal: botões `0xc3f` Single Player, `0xc40` Multi Player, `0xc41` Editor, `0xc42` High Scores, `0xc43` Credits, `0xc44` Quit (ids 3135..3140 de `screens.md`; LNG 1674..1685) |
| `5` | **K1** menu multiplayer: botões `0xc4a` TCP/IP (provedor 2; campo de IP `0xc94`, LNG 1717 "Server IP: (Leave blank to search)"), `0xc4b` IPX (3), `0xc4c` Modem (4), `0xc4d` Serial (5) |
| `6` | **K2** (depois de o provedor iniciar): TCP/IPX: lista de servidores `0xc91` com linhas `"nome  N Player(s)"` (LNG 2860), Join `0xc3c` (LNG 1707; sem servidor escolhido: LNG 374/375), "Change" `0xc9d` (LNG 1704; abre o diálogo do perfil, H3, `0x4ac510`), Create `0xc3d` (LNG 1709; `0x4b65d0` pede o nome do servidor, LNG 517, e pergunta LNG 373 se já hospeda), Refresh `0xc3e` (LNG 1711, `0x4b6420`); Modem/Serial: Host `0xc50` (LNG 1713; cria a sessão `RT2_Host` e espera o convidado, LNG 518) e Guest `0xc51` (LNG 1715; tenta entrar a cada 3 s, `0x4b73a0`) |
| `7` | **K3** preparação do jogo (conectado): lista de jogadores `0xc90`, contador de computadores `0xc81`/`0xc82`, horses de dificuldade `0xc8a..0xc8e`, modelos `0xcaa..0xcac`/`0xcc2..0xcc4`, sandbox `0xcc5`, modificadores de receita `0xcb0`/`0xcb2`, Options `0xc4e` (abre as páginas `0xa`/`0xf`), chat `0xc97`/`0xc98`, botão Start `0xc3a`, Back `0xc3b` |
| `8`/`9`/`0xa`/`0xe`/`0xf` | escolha de mapa ou jogo salvo e opções avançadas (A3, A5) quando conectado (`8`, `9`, `0xa`) ou não (`0xe`, `0xf`) |
| `0xb`, `0xc`, `0xd`, `0x10`, `0x11`, `0x12` | editor, carregar jogo/cenário/campanha (`screens.md` A, B, L) |

Sair (Back `0xc3b` ou qualquer botão `0xc3f`..`0xc44` do menu principal) com 2 ou mais jogadores pergunta (LNG 187): "If you do this, you'll disconnect from your multi-player game.\n\nThere are currently %1 other human(s) connected.\n\nGo ahead and disconnect?".

### 2.2 O bloco de configuração `0x574840` (0xb6 bytes)

É o mesmo bloco do save (`map-gam.md`, arquivo `0xB221`); em rede ele é a **fonte única** e viaja inteiro no comando de lobby `5`. Layout [S]:

| offset | endereço | conteúdo |
|---|---|---|
| `+0x00` | `0x574840` | nº de jogadores de computador |
| `+0x01` | `0x574841` | cavalo de dificuldade 0..4 (Easy, Medium, Hard, Expert, Custom) |
| `+0x02` | `0x574842` | nome do mapa ou jogo (30 caracteres) |
| `+0x21` | `0x574861` | 1 = jogo salvo, 0 = mapa novo |
| `+0x22` | `0x574862` | ano inicial u16 |
| `+0x24` | `0x574864` | identificador/checksum do mapa u32 |
| `+0x28..+0x2d` | `0x574868..` | (mínimo e máximo de jogadores no total), (mínimo e máximo de humanos), (mínimo e máximo de computadores), do cabeçalho do mapa (`0x567c88..0x567c9c`) |
| `+0x2e` | `0x57486e` | `game_seed` u32: sorteado quando o anfitrião abre o lobby (`0x4b9ff0`, como `rand(1,32000) + clock_ms` [S]) e distribuído pelo comando de lobby `5` a cada entrada; **não** viaja no `4` |
| `+0x32` | `0x574872` | opções: bits 0..2 modelo **industrial** (0 básico, 1 avançado, 2 expert), bits 3..5 modelo **financeiro**, bit 6 modo **sandbox** |
| `+0x34`, `+0x35` | `0x574874/5` | modificador de receita dos **humanos** e dos **computadores** (0..10, 5 = nenhum; cada passo = 5 %, de -25 % a +25 %; texto `"-"`/`"+"` + `|v-5|*5` + `"%"`, ou LNG 188 "None") |
| `+0x36 + 4k` | `0x574876..` | 32 vagas de jogador de 4 bytes: persona, índice do humano (-1 = nenhum), índice do computador (-1 = nenhum), flags (bit 0 = vaga em uso) |

Predefinições dos cavalos (`0x4b3e00`, tabelas `0x55af5c` humanos = 9, 5, 3, 1, 3 e `0x55af60` computadores = 3, 5, 7, 9, `0x2e`): cavalo 0 zera os dois modelos, 1 = (avançado, avançado), 2 e 3 = (expert, expert), 4 (Custom) não mexe; sempre limpa o sandbox. A cada mudança manual, se o conjunto deixa de coincidir com a predefinição do cavalo atual, o cavalo vira **Custom** (`0x4b4de0`). "Difficulty Rating: N %" (`0x4b4de0`/`0x4b51e0`) **só é mostrado fora de rede** (conectado: texto vazio) [V]; fórmula (valor em `%`): `fin = {0: 10, 8: 40, outro: 50}` (bits 3..5), `base = ftol(fin * 1,7)` se industrial = 1, `fin * 2` se = 2, senão `fin`; `t = ftol(mod_comp * 0,1 * (computadores * 10) + base)`; `rating = ftol(t * (15 - mod_humano) * 0,1)`; texto = `rating * 0,01` (constantes `0x5353b0` 1,7, `0x535178` 0,1, `0x5350b8` 0,01).

### 2.3 Comandos de lobby (ids 1..9 do mesmo sistema de comandos)

Antes do início (`0x578b00 == 0`) `cmd_queue_run` e `0x4af500` desviam para `net_game_start` (`0x4b69b0`), que **executa a fila de lobby**; os pacotes passam pelo mesmo caminho cliente -> anfitrião -> lote -> todos (1.5). O carimbo `+8` é ignorado aqui. Cada comando que muda a configuração (2, 3, 4, 5, 7) **apaga o bit de aprovação de todos os jogadores** (`+0x2a &= ~1`) e redesenha a lista de jogadores (`0xc90`).

| id | tamanho | corpo (`+0xc`) | efeito |
|---|---|---|---|
| `1` | `0x10` | id de rede u32 | `net_player_left(id)` (seção 5) |
| `2` | `0xd` | cavalo 0..4 (`id_do_controle - 0xc8a`) | `0x4b3e00`: aplica a predefinição |
| `3` | `0xd` | delta i8 (+1/-1) | altera o nº de computadores se couber nos limites (humanos e total dentro de `[min,max]`; total < 32); senão ignora. Quem pediu e não coube vê LNG 379 "You cannot add any more computer players for this scenario." ou LNG 380 "You must have at least your current number of computer players for this scenario." |
| `4` | `0x38` | bloco de 44 bytes a partir de `0x574842` (`+0x02..+0x2d`: nome, tipo, ano, id do mapa, limites; **sem** `game_seed`) | escolhe mapa ou jogo salvo; no registro do anfitrião liga os bits 1 e 2, nos outros desliga o bit 1; cada cliente que **não** é o anfitrião responde com o comando `9`. `0x4b68a0` carrega o cabeçalho do mapa localmente. Mensagens: LNG 2208 " (Invalid)", 2210 " (Wrong # of players)", 2698 " (Saved Game)" |
| `5` | `0xc2` | bloco inteiro de 182 bytes (`0x574840`) | usado pelo anfitrião para dar a configuração completa ao convidado que acabou de entrar (`0x4b9f80`); mesmo efeito do `4` e responde com `9` |
| `6` | `0xd` | vazio | **iniciar**: ver 2.6 |
| `7` | `0x10` | `+0xc` = bits 0..2 (industrial), `+0xd` = bits 3..5 (financeiro), `+0xe`/`+0xf` = modificadores de receita | muda os modelos (`0xcaa..0xcac` mexem no financeiro, `0xcc2..0xcc4` no industrial) e os modificadores; o sandbox é mudado localmente pelo anfitrião (`0xcc5`, `^= 0x40`) e vai no `5` |
| `8` | `0x10` | id de rede u32 | **alterna a aprovação** (bit 0) do jogador |
| `9` | `0x15` | id u32 do remetente, id do mapa u32, flags (bit 0 = é jogo salvo, bit 1 = o jogador **tem** o arquivo) | confirma que o jogador viu a mesma configuração (`id do mapa` e tipo iguais): grava `bit1 = 1` e `bit2 = bit1_da_msg` |

Apenas o anfitrião deveria emitir `2`, `3`, `4`, `5`, `6`, `7`: a interface esconde o botão do mapa (`0xc4f`, "Only the host computer can choose the map or saved game") para convidados, mas o protocolo **não valida o remetente** [S]. A aprovação (`8`) só passa pela interface para a própria linha: `0x4b74f0` recusa com LNG 2212 "You can't toggle this for other players!" e, se a configuração não é válida (`DAT_0055af58 != 0`), com LNG 2213 "The current map is invalid for you."

### 2.4 Vagas, humanos e computadores (limites)

- Total de jogadores (humanos + computadores) **no máximo 32** (`0x1f < n` recorta em `0x20`); o cabeçalho do mapa dá `[min, max]` do total, dos humanos e dos computadores (`map-gam.md`: obrigatórios, total, obrigatórios que não aceitam computador, que aceitam humano, obrigatórios que não aceitam humano, que aceitam computador). O mapa também fixa, por vaga, persona, se é obrigatória, e se aceita humano e/ou computador (LNG 21 e 22: "must be in the game or optional", "human, computer or either"); regras LNG 2290 "The first player is always mandatory." e 2291 "The first player may not be an AI only player."
- `0x4b45b0(1)` (`setup_assign_player_slots`) recorta o nº de computadores aos limites e reatribui as vagas até o total de humanos e computadores caber (até 99 passadas): os humanos ocupam as vagas na ordem do **roster** (índice 0 = menor id); computadores vão para as vagas que aceitam IA; vagas sem persona recebem uma persona sorteada com `rng_hash_range(game_seed, 0, n-1)` entre as personas livres do conjunto `0x567d40` (`0x4b4360`). Se os limites de humanos não se satisfazem, `DAT_0055af58 = 3` (" (Wrong # of players)") e ninguém consegue aprovar (2.3).
- Mistura com IA: sim, livre dentro dos limites do mapa. Humanos novos que entram **depois** de fixada a lista disparam `0x4b3ec0`, que reajusta o nº de computadores para caber; se não couber, zera os computadores.
- Modem e Serial: o código **não** limita a dois jogadores; o limite vem do provedor DirectPlay (manual: "um contra um").
- Só o anfitrião escolhe mapa e opções (interface); todos aprovam; o ping de cada jogador aparece na linha (`registro+0x10`, 0 para si mesmo); LNG 2220 "The ping time, in milliseconds, between your computer and this computer."

### 2.5 Linha da lista de jogadores K3 (`0x4b9990`, `0x4b3ec0`)

Cada humano é uma linha (índice `| 0x8000`), cada computador outra (LNG 2700 "Computer Player"): luz de **aprovação** (sprite `0x16 + bit0`), luz de **arquivo** (sprite `0x1a + estado`: 1 sem verificação, 2 sem o arquivo, 0 com ele; LNG 2699 explica que o arquivo será enviado, lento pela internet), **nome** do perfil, **memória** em MB (`+0x28`), **ping** (LNG 2593). A linha do jogador local tem contorno verde (LNG 2857). O botão de início só avança com todas as luzes acesas (2.6).

### 2.6 Início

`0xc3a` (Start): com 2 ou mais jogadores confere que **todos** os registros têm o bit 0; senão LNG 2216 "You can't start the game until all human players have approved of the game settings...". Envia o comando `6`. `net_game_start`, caso `6` (em todas as máquinas, na ordem do lote):

1. LNG 2211 "Launching Game..."; `0x49ce00` dá 45 s de folga aos temporizadores de todos os outros jogadores (o carregamento é lento); `0x49cdb0` **fecha a sessão** para novos jogadores (DirectPlay `JOINDISABLED`, `0x4c6930`); o anfitrião envia o arquivo a quem falta (`0x4b3640`, 1.9).
2. Quem é convidado e ainda não tem o arquivo espera (LNG 2693) até tê-lo (LNG 2694 "Loading game...").
3. `DAT_00578b00 = 1`, `rng_seed(game_seed)`; mapa novo: `0x479b20(nome)` (`world-tick.md`); jogo salvo: `0x495700(0)` e `0x479710(nome, 0)` (atribui os humanos às pessoas, 5.3); em seguida `0x4b34c0` (barreira, 1.7) e `0x574c18 = 1`.
4. Os humanos viram **pessoas** (`0x473590`): para cada vaga em uso `pessoa.+0x122 = (índice_do_humano >= 0)`; `+0x126 = id de rede do humano` (0 se jogo local, -1 para IA); `+0x52f` = persona; nome (`+8`) e descrição (`+0x27`) copiados da persona (LNG) e, para humanos em rede, **substituídos pelo perfil do jogador** (nome, descrição e retrato `+0x132`); a pessoa do jogador **local** vira `local_person`/`local_company` (`0x467bd0`, `map-gam.md`; `0x46aea0` dá o índice do jogador local no roster).

## 3. O que muda na simulação com vários humanos

"Rede" = `net_player_count() >= 2`. Todas as linhas abaixo são regras do EXE [V] salvo indicação; as que o clone precisa replicar estão na seção 6.

### 3.1 Tempo, pausa e velocidade

| regra | endereço |
|---|---|
| **Diálogos não pausam o jogo**: fora de rede `dt = 0` quando há diálogo modal aberto (`0x57fee4 != 0`) e a opção "Keep game going in dialogs" (`opts+0x50`, LNG 2086/2087) está desligada; **em rede o `dt` é sempre calculado** ("In multi-player games, this is always the case") | `0x405540` (`0x4055a9..0x4055c8`) |
| velocidade e pausa são o comando `0x0b` (`commands.md`): qualquer humano emite (teclado, controle deslizante, `settings.win`), o anfitrião ordena e **todas as máquinas mudam**; não há votação nem aprovação; executa na chegada, não no minuto carimbado | `0x4af6d0`, `0x4afd50`, `0x47beb6`, `0x4531f0` |
| `train_crash` pode forçar a velocidade para 4 (`0x7f, 4`) se a atual é maior: em rede é um comando emitido pela máquina onde o trem bateu [S] | `0x4ce9ab` |
| **velocidade 0 ("Paused") em rede não congela**: `minutos(1000 ms) == 0` leva a `dt = H - T`, e o anfitrião só concede `T + 0 + 1`; o relógio avança **1 minuto de jogo por segundo real** (um dia a cada 24 min reais) | `0x405540`, `0x4af8c0` |
| no início da partida em rede o `cmd 0x0b` é emitido por `0x49e046` (e depois do aviso do corretor, se a velocidade > 4: `0x4a7b9c`) | `commands.md` |
| o anfitrião não deixa o relógio passar do horizonte; os convidados ficam parados no horizonte se o anfitrião atrasar (sem lote por > ~2 s) | 1.5 |

### 3.2 Diálogos, jornais e mensagens

- **Fecham sozinhos em rede.** `0x4510a0` (caixa de mensagem genérica; último parâmetro = tempo em ms) passa o tempo a `0x501ce0` (laço modal; `param_6`) **somente se** `opts+0x1c3` ("Dialogs disappear if left alone", LNG 2617/2618) ou `net_player_count() >= 2`; senão 0 (espera para sempre). Ao estourar o tempo o diálogo devolve 0 (como fechar sem escolher); **qualquer evento de interação** (tipos 2, 4 ou 7 do laço: mouse/tecla) cancela o tempo. Tempos em rede: `5000` ms nos avisos "A new building is available to be built in stations: ..." (LNG 67, `0x405b70`, `0x406080` mês 9), `8000` ms nos diálogos da fila de mensagens (`0x405ec0` casos 0 e 4) e na caixa da indústria `0x4e3620` (`0x1f40`); `6000` ms no jornal (`0x4a4050`). O aviso de acidente "You've had a train wreck! ... replace this train" (LNG 2900, fila tipo 6, só para a empresa local) é uma escolha e **não** tem tempo. Os **votos** (fusão `0x48b2f0`, eleição `0x42eb40`) passam 0: **não fecham sozinhos** e a apuração espera todas as pessoas humanas responderem (`finance.md` 15); sair da partida entrega as pessoas à IA, que vota por sorteio.
- **Escolha de evento (`+0x60 == 2` da regra, dois botões com efeitos)**: fora de rede abre o diálogo (`0x4510a0`) e o jogador decide (botão 2 = `0x67` aplica o conjunto de efeitos `k = 1`); **em rede o diálogo não é mostrado e o resultado é sempre o conjunto `k = 0` (primeiro botão)** (`event_show_message` `0x455e70`) [V]. Mensagem tipo 0 (diálogo) entra na fila da máquina (`0x469320` tipo 4, aberta com 8 s em rede) e tipo 1 (jornal) vai a `0x4a42f0`.
- **Briefing e eventos de início**: o gatilho 6 (briefing) corre em **todas** as máquinas ao montar o mundo (`0x479b20`), sem aplicar efeitos; o texto só aparece para quem é a pessoa/empresa do evento (ou para pessoa e empresa 0), como diálogo comum (auto-fecha em 8 s em rede). O botão de briefing no jogo (`0x48c370`, `0x1fb`) continua disponível; reiniciar o cenário (`0x1fa`) não (3.5).
- **Fila tipo 2** (`0x452410`): caixa com tempo de 4000 ms em rede [?: que mensagem usa esse tipo].
- **Jornal e relatório anual.** O jornal de mudança de economia (`0x46a010`) e a notícia de empresa nova (`0x43d800`) só abrem como popup com `opts+0x228 == 0` (opção "Newspapers") **e** `net_player_count() < 2`; em rede vão para a lista de mensagens (`0x46adb0`) ("In multi-player, news frequency is always very low", LNG 2672..2676). O relatório anual (`0x4ebbd0`, `0x406080`) só abre com `opts+0x228 != 2` e `net_player_count() < 2`. Na tela de detalhe da pessoa (`0x4a9510`) o botão "Change" (nome, descrição e retrato da pessoa local, LNG 2911/2912) só aparece com `net_player_count() < 2`: em rede o perfil só se edita antes de iniciar (H3).
- **Mensagens por jogador**: cada máquina tem a **sua** fila de mensagens (`0x574c1c`), preenchida com o que concerne `local_person`/`local_company` (`scenarios.md` 2.4); isso é interface e não entra no checksum.
- **Eventos do editor com filtro de rede** (`+0x351` da regra): `0` qualquer partida, `1` só partida local (um humano), `2` só rede, ignorado em campanha (`scenarios.md` 2.1); o editor tem as condições `GameIsMultiPlayer`/`GameIsNotMultiPlayer` (`0x63`/`0x64`, `0x573f1c`), `PlayerControlsAnyCompany`, `PlayerControlledCompanyId` e as opções "Test against the viewing player only" (LNG 3373..3376: avaliação local `+0x34f`/`+0x350`, sem aplicar efeitos e com o `rand` do CRT, não o RNG da simulação).

### 3.3 Condições de vitória, derrota e fim do cenário

- **Efeitos "Game Over" 1..3 (bronze/prata/ouro) e 4 (derrota)** (`rule_evaluate` `0x455f30`, `scenarios.md` 4.2): o alvo é a pessoa da regra (ou o chairman da empresa, ou a única pessoa). Se o jogo ainda não acabou (`[0x573d70] == 0`): **derrota (4)**: se o alvo é a pessoa local (ou 0) o estado local vira 4, senão `0x4a6a00(alvo)` **elimina esse jogador** sem encerrar o jogo (a pessoa some, `finance.md` 14.4); **vitória (1..3)**: se o alvo é a pessoa local o estado vira o nível e o texto é o da regra; **senão o estado local vira 4 com o texto LNG 2691 "Sadly, another player has achieved this scenario's goals before you."** Todas as máquinas avaliam a regra no mesmo minuto (é parte do lockstep), então o primeiro a cumprir a meta ganha e todos os outros perdem na hora.
- **Tela de fim** (`game_over_screen` `0x4e9280`): fora de rede pergunta LNG 2207 "Do you wish to keep playing the current game?" e grava a pontuação (`0x4701f0`) quando não é campanha; **em rede** mostra só LNG 2862 "The game has been won, but you all may continue to play on the map as long as you wish." e **continua o jogo**; sem pontuação. Não há desempate: vale quem dispara primeiro na ordem de avaliação (ordem das pessoas e empresas, `scenarios.md` 2.3).
- Campanha: o cenário em rede não avança a campanha (`+0x351` e `[0x570234]`).

### 3.4 Economia e empresas

- **Falência.** A opção "Remove track and stations of bankrupt companies" (`opts+0x240`, LNG 2915) só vale com `net_min_peer_version >= 0x232d` e `net_player_count() < 2` (`0x43c760`, chamada de `0x438170(empresa, remover)`); **em rede a via e as estações ficam no mapa** [V].
- Modelos econômicos, modificadores de receita (humano/computador) e computadores são os do bloco de configuração (2.2) e valem **iguais para todos os humanos**.
- Pessoas e empresas: cada humano controla uma pessoa (`+0x122 = 1`, `+0x126 = id de rede`); os comandos de cada empresa carregam o id da empresa no corpo (`+0xc`), e o código não confere quem manda (6, 9). `local_person`/`local_company` (`0x566ca4`, `0x574414`) são **por máquina** e só afetam interface e as poucas linhas de "estado local" abaixo.
- **Dependências do jogador local que tocam estado da simulação** (fontes de dessincronia latente do original, `commands.md` 3.3): construir estação com `P.empresa == local_company` apaga o bit do tamanho nas melhorias (`estação+0x49 &= ~(1 << tamanho)`, `0x409a30`): **nas outras máquinas o bit não é apagado** [S]. O clone deve aplicar a regra a **toda empresa de pessoa humana** (idêntico ao original com um humano; determinístico com vários).
- Votações (fusão `0x23`, eleição `0x25`): ver 3.2; "Numa partida em rede de versão >= 0x232d, uma empresa inativa aborta a votação" (`finance.md` 15).
- IA: `ai_*` roda em todas as máquinas, igual; as pessoas de IA são as de `+0x122 == 0`; uma pessoa humana que sai vira IA (5.1).

### 3.5 Recursos desativados em rede

| recurso | regra | endereço |
|---|---|---|
| cheats ("bigfootgold", "bobo", "king of the hill", "cattle futures", "powerball", "slush fund", "let me in"; LNG 2722..2731) | `0x46c8a0` retorna se o roster tem 2+ jogadores (também liga `0x574b74`, o multiplicador de velocidade de trem de `0x4cf030`); o chat não os interpreta | `0x46c8a0` |
| teclas de depuração de trem (B, C, R com o modificador de bit 2; agem sobre o trem selecionado `0x574780`: `0x4ce4c0`, `0x4ce730`, `0x4ceaa0`) | só com `net_player_count() < 2` [S] | `0x401884` (casos `0x42`, `0x43`, `0x52`) |
| alternar o editor (tecla E com modificador; só em build de depuração, `0x55d224 > 1`) | só com um jogador | `0x401884` (caso `0x45`) |
| carregar jogo salvo pelo menu do jogo | LNG 2738 "You must be in the multi-player setup part of the main menu to load a game in multi-player play." | `0x48c1b0` |
| reiniciar o cenário (briefing) | LNG 2745 "This option is not available in multi-player mode." | `0x48c370` |
| salvar jogo | **permitido**, local a cada máquina (sem comando); o save guarda a identidade de cada humano (5.3) | `0x48c150`, `0x477540` |
| "Difficulty Rating" no lobby | oculto em rede | `0x4b4de0` |
| multiplicador de velocidade de trem do cheat `0x574b74` | só com um jogador | `0x4cf030` |

## 4. Mensagens de rede, chat e textos

### 4.1 Chat

TAB abre a linha de entrada (manual: "TAB, digitar, ENTER"; `0x46c4a0` abre `chat.win`, controle `0x1325`, e não tem chamador direto no código, então o atalho TAB vem por tabela de aceleradores ou por salto indireto [?]); ENTER envia. Com 2+ jogadores: `0x49cd20` manda a mensagem `4` (texto, até 900 caracteres, não confiável) a todos e mostra a linha localmente; com 1 jogador o texto vai para o interpretador de cheats (`0x46c8a0`). Formato da linha: `{[HJL]%s:  %s}` (`0x554634`; código de justificação `[HJL]` = esquerda), com o **nome do perfil** do remetente (`registro+0x2b`); no lobby é o campo `0xc97` (`0x4b66f0`, controles `0xc98` entrada, `0xc99` enviar; LNG 1721..1725). Em jogo as linhas aparecem sobre o mapa: até 6 linhas, cada uma some 14 s depois de aparecer (`0x46c0b0`, `0x46c310`); há opção de voz "Speak Train Events & Chat Messages" só na versão Mac (LNG 3440). Não há nome de destinatário: tudo é para todos.

### 4.2 Textos da interface (ids de `default2.lng`)

| tela | ids |
|---|---|
| K1 menu | 1676 Multi Player, 1677, 1694/1695 Gathering Net, 1696/1697 TCP/IP, 1698/1699 IPX, 1700/1701 Modem, 1702/1703 Serial, 1704/1705 Change, 1717/1718 Server IP, 1719/1720 Your IP |
| K2 servidores | 1706 Servers, 1707/1708 Join, 1709/1710 Create, 1711/1712 Refresh, 1713/1714 Host, 1715/1716 Guest, 456 "Searching for servers, please wait...", 373 "Do you want to drop your old server?", 374 "There are no servers to join.", 375 "You haven't selected a server to join.", 517 nome do servidor, 518 "Waiting for guest to connect, please wait...", 2861 "Establishing multi-player connection.  This might take a while...", 2916 "(Connected)", 2860 "Player(s)" |
| K3 preparação | 1726/1727 Players, 1747/1748 Computer Players, 2216 aprovação, 2217 tipo, 2218 nome, 2219 memória, 2220/2593 ping, 2221 luz de aprovação, 2699 luz de arquivo, 2700 Computer Player, 2857 lista, 2208 "(Invalid)", 2209 "(You don't have this map)", 2210 "(Wrong # of players)", 2211 Launching Game, 2212, 2213, 2698 "(Saved Game)", 2651..2657 "Start/Load ... for this multi-player game", 379, 380, 186 "Difficulty Rating: %1" |
| H3 personagem | 1704/1705 Change, 2912 "Change your name, description, and/or portrait" |
| chat | 1721 Chat, 1722, 1723, 1724 Send, 1725 |
| durante a carga | 2692 Waiting for other players, 2693, 2694, 2695, 2696, 2706, 2707, 2865 |
| em jogo | 2319 "Player %1 has exited the game.\n\nHis position will be assumed by a computer player.", 2320 "Out of Synch", 2691, 2862, 187 |
| erros | 2259 "Unable to load Direct Play DLL (DPLAYX.DLL).  Try re-installing Direct X 5.0.", 2932 "Unable to initialize Direct Play.", 2919, 2918, 457, 372, 382, 383, 381, 378 |
| aviso de compatibilidade | 3319: o Platinum é **incompatível em rede com o RT2 original**, mas compatível com o Second Century e o Gold; mostrado uma vez (`opts+0x241`) ao escolher TCP/IP |

## 5. Desconexão, entrada tardia, salvar e carregar

### 5.1 Quem cai

- **Detecção** (`0x49ce50`, a cada >= 100 ms, chamada pelo relógio): para cada jogador remoto do roster (o **anfitrião** faz isso com todos; um **convidado** só com o anfitrião, e nunca entre convidados): (a) se **nada foi enviado a ele há 10 s**, manda `0xd` (batimento); (b) se **nada foi ouvido dele há 23 s** e já passaram 5 s da última sonda, manda `0xe`; (c) se **nada foi ouvido há 90,001 s** (`0x15f91` ms) e `Network Ignore Heartbeats` está desligado: o jogador é **removido**. Quando (c) pega o anfitrião, **cada convidado** executa a migração por conta própria e todos escolhem o mesmo sucessor; a remoção (`cmd 0x01`) pode sair em duplicata e é idempotente (a segunda não acha o registro). `0x49ce00` dá +45 s de folga ao "último ouvido" de todos quando um jogo começa ou carrega.
- **Saída voluntária**: `0x49c770` manda `0xc`, esvazia a rede e volta ao menu; o par que a recebe (`0x49d6f0`) faz o mesmo que na detecção.
- **Remoção** (`0x49d6f0`/`0x49ce50`): se o que caiu é o anfitrião, escolhe o novo (o primeiro id diferente no roster, que é o de menor id), manda `9`, e o novo anfitrião passa a conceder lotes (`0x579230`: o horizonte nunca recua e ganha +8 s de folga, 1.5). Depois: com **menos de 3 jogadores** chama `net_player_left(id)` direto em cada máquina; com **3 ou mais** emite o comando `0x01` (`cmd_player_left`) pelo lote, para a remoção cair no **mesmo minuto** em todos.
- **`net_player_left`** (`0x49d970`): tira o registro do roster; fora do lobby: mensagem LNG 2319 (diálogo na fila de mensagens, auto-fecha em 8 s) e, para cada pessoa com `+0x122 != 0` e `+0x126 == id`: `+0x122 = 0` (a pessoa passa a **ser de IA**); se `net_min_peer_version >= 0x232b` também `+0x52f = 0`, `+0x548 = 3`, `+0x267 = 0` (a IA pode agir de imediato). A **empresa e o dinheiro ficam**; não há liquidação.
- Se sobra um jogador só, `net_player_count()` volta a 1 e todo o comportamento "fora de rede" retorna (diálogos esperam, `cmd_send` vira local, trens voltam ao passo por quadro, RNG deixa de ser re-semeado); a sessão DirectPlay continua aberta mas fechada para entradas.

### 5.2 Entrar no meio e reconectar

**Não existe.** A sessão é fechada ao lançar (`JOINDISABLED`, 2.6) e a mensagem `5` só é tratada com `0x578b00 == 0`. Um jogador removido (queda, 90 s sem sinal) perde a vaga para sempre; o manual manda salvar com frequência para "reconectar e recarregar se alguém perder a conexão" (carregar o save pelo lobby, 5.3).

### 5.3 Salvar e carregar

- **Salvar** (`0x477540`): cada máquina salva localmente. Para cada pessoa, `+0x54d` recebe a **identidade persistente** do humano que a controla (`[0x57b400]+0x3c`: o próprio id para a pessoa do jogador local, ou `registro+0x24` do dono em rede; 0 para IA). `map-gam.md` 5 (`+0x54d` é o campo da pessoa).
- **Carregar em rede** (`0x479710`, só pelo lobby, `screens.md` K3): para cada pessoa humana (`+0x122 != 0`), três passadas: (0) casa `pessoa.+0x54d` com `registro.+0x24` de um jogador conectado ainda sem pessoa; (1) as pessoas humanas restantes recebem, em ordem, os jogadores restantes; (2) humanas sem jogador viram IA (`+0x122 = 0`). A pessoa do jogador local vira `local_person` (`0x467bd0`). Depois `0x4b34c0` (barreira) e `0x574c18 = 1`. Resultado: quem estava no save reassume a **mesma** pessoa, mesmo com outro id de rede.
- **Recuperação de "Out of Synch"**: não é automática; o `readme3.txt` (M) manda o anfitrião salvar e todos relançarem o jogo desse save.

## 6. Implicações para o clone

**Precisa replicar (regras)**

1. Modelo **host sequenciador + comandos**: toda mutação passa por `Command` (ids de `commands.md`), com lote carimbado `(stamp, horizonte)`; execução no minuto carimbado, em ordem de lote, nunca no passado; a simulação limitada ao horizonte; o anfitrião também executa o próprio lote pelo mesmo caminho. A latência de projeto do original é **2 s reais de jogo** de folga (`H = T + minutos(2000 ms) + 1`) e batimento de 1 s; os parâmetros 10,0/3,0 e 1/3 de alcance (1.5) são calibragem de interface e podem ser trocados.
2. Em rede o **RNG é re-semeado todo dia** com `T + game_seed + caixa da pessoa 0`, os **trens andam em passos de 1440 min** e `net_game_start` semeia com `game_seed`: a simulação em rede **não** é a de um jogador (`time.md`, `trains.md`, `rng.md`). O modo "rede" do `rt2-sim` precisa dessas duas diferenças para os mesmos números.
3. Velocidade/pausa = comando global de qualquer humano; **sem pausa por diálogo**; diálogos não bloqueantes com os tempos de 3.2 (5/6/8 s; votos sem tempo, com a apuração esperando todos os humanos); jornal e relatório anual desligados; falência não remove trilho; cheats, editor, recarregar e reiniciar bloqueados; "Difficulty Rating" oculto. A pausa em rede avança o relógio 1 minuto por segundo.
4. **Vitória**: o primeiro a cumprir a meta vence e os outros recebem derrota com LNG 2691; derrota de um jogador elimina só ele; o jogo continua depois da vitória (LNG 2862); sem recorde.
5. Lobby: bloco `0x574840` (2.2), os comandos `2..9` (2.3), aprovação (bit 0) que **zera a cada mudança**, limites do mapa, persona de vagas sem dono por `rng_hash_range(game_seed)`, perfil (nome, retrato, descrição) do jogador, atribuição das pessoas (2.6), barreira de carga, transferência de mapa/save e bloqueio de entrada depois do início.
6. Desconexão: batimento 10 s, sonda 23 s, corte em 90 s (45 s de folga ao carregar), migração do anfitrião para o menor id, remoção sincronizada (comando `0x01` quando restam 2+), pessoa humana vira IA (+ `+0x52f = 0`, `+0x548 = 3`, `+0x267 = 0`), empresa preservada; um humano restante volta ao modo local. Salvar guarda a identidade do humano (`+0x54d`); carregar casa por identidade, depois por ordem, depois IA.
7. Chat: broadcast, 900 caracteres, formato `Nome:  texto`, 6 linhas com 14 s, TAB abre.
8. **Corrigir sem romper**: aplicar o apagar-bit da estação a toda empresa humana (3.4), e validar o remetente de cada comando contra a pessoa que controla a empresa (a flag deve ficar atrás de uma opção de modo fiel, `original-bugs.md`): o original confia em qualquer cliente.

**É livre (não afeta regra)**: o transporte (DirectPlay, IPX, modem, serial; fragmentação de 248 bytes, CRC, ack, tipos `1..3`, tabelas de reenvio); como o ping é medido (só precisa de uma medida em ms por jogador); a escolha de TCP ou QUIC e a descoberta de servidores (LAN, IP direto); o formato exato do pacote de lote, desde que carregue `(seq, stamp, horizonte, comandos)`; a codificação do arquivo de mapa na transferência (o original usa um compressor próprio e CRC); o mecanismo de barreira, desde que todos terminem de carregar antes do primeiro lote; e os textos de depuração/estatística.

**Convém melhorar** (sem mudar regra): checksum de estado completo e periódico com **reação sempre ligada** e *dump* de diferença (o original só detecta com depuração e nunca corrige), reenvio do estado a quem divergiu, e reentrada por reconexão (o original não tem).

## 7. Divergências entre o código e o manual / readme

| item | manual ou readme | código |
|---|---|---|
| jogo em rede | "play is identical to the Single Player mode" (cap. 9) | trens por dia, RNG por dia, popups com tempo, sem jornal, sem cheats/editor (3, 1.6) |
| "Out of Synch" | `readme3.txt` (M): o jogador vê a mensagem e o conserto é salvar/relançar | a mensagem, o novo `cmd_seed_rng` e a pausa só existem com "Network Debug Mode" (desligado); sem ele, silêncio (1.8) [?: o readme descreve um build onde isso estava ligado] |
| Alt+Tab | "irregular behavior... disconnected" | sem polling da rede os batimentos param; 90 s depois o jogador é cortado (5.1) |
| modem/serial | um contra um | sem limite no código (do DirectPlay) |
| mapa só do anfitrião | "Only the host computer can choose the map or saved game" | a interface esconde o botão; o protocolo não valida o remetente |
| pausa | PAUSE congela | em rede o relógio anda 1 min/s com velocidade 0 (3.1) |
| aprovação | "if any option is changed after a player clicks approval, the light turns off" | confere (2.3): toda mudança apaga as luzes de todos |

## 8. Em aberto

- `DAT_0055af58` (estado da configuração): vi gravarem 0, 1 (mapa inválido) e 3 (número de jogadores); não achei quem grave 2 (" (You don't have this map)", LNG 2209). `0x4b74f0` só aceita aprovar com 0. Provável código morto.
- Quem pode apertar **Start** (`0xc3a`): o protocolo aceita de qualquer par; a visibilidade do botão para convidados na página `7` não foi decidida [?].
- Efeito de o diálogo da fila tipo 2 (`0x452410`, 4000 ms) estourar o tempo (devolve 0): não segui o chamador que usa o resultado.
- O papel do campo `+4` do registro de sessão em `0x4f` bytes e dos 20 bytes seguintes de `0x44ffb0` ("DPServer"): só relevantes ao DirectPlay.
- Se `readme3.txt` (M) corresponde a um build com `Network Debug Mode` ligado por padrão (o EXE analisado zera `[0x57b400]+0x2c` em `net_session_init`). Teste dinâmico: rodar duas instâncias no Wine e forçar divergência.
- `0x4cf030` e o cheat `0x574b74` (multiplicador de velocidade de trem só com um jogador): só confirmado o teste de rede; a fórmula de cheat não foi especificada.
- A tabela de personas disponíveis no pool (`0x567d40`) e a ordem exata de `0x4b4360` com humanos e computadores misturados (rascunho em 2.4).
- Caminho do jogo lançado por lobby DirectPlay (`/A<n>`, `DAT_00574b70`, `0x450080`, ramos em `0x44ec10`, `0x44ef60`, `0x44fac0`, `0x4c68e0`): lido só por cima, não especificado.
- Identificação do modo `DAT_00566e14 = 0xb` ("lobby conectado") versus `0xc`: só consta que os dois param o avanço de tempo (`0x405540`).

## 9. Endereços de referência

`0x44ec10` dp_init · `0x44ef60` dp_select_provider · `0x44f730` dp_create_session · `0x44fac0` dp_join_session · `0x44fda0` dp_send · `0x44fe10` dp_receive_pump · `0x49c770` net_shutdown · `0x49c9a0` net_start_provider · `0x49cac0` net_join_session · `0x49cc50` net_create_session · `0x49ce50` net_heartbeat · `0x49d240` net_roster_add · `0x49d6f0` net_player_exit · `0x49db90` net_host_changed · `0x49de20` net_sync_check · `0x49e0b0` net_time_sync · `0x4a0410` net_send · `0x4a0540` net_dispatch · `0x4a1d80` net_recv_packet · `0x4af500` net_host_receive_command · `0x4af6d0` cmd_send · `0x4af8c0` cmd_net_flush · `0x4afc80` net_batch_receive · `0x4afd50` net_batch_apply · `0x4afee0` time_horizon · `0x4b34c0` net_barrier_loaded · `0x4b3640` net_send_map_file · `0x4b51e0` setup_set_page · `0x4b69b0` net_game_start · `0x4b75b0` setup_win_handler · `0x46c4a0` chat_open_input · `0x46c0b0` chat_show_line · `0x4510a0` ui_message_box · `0x501ce0` ui_modal_run. Globais: `0x577dc4` net_local_id, `0x577dc8` net_is_host, `0x577db8` net_provider, `0x577dbc`/`0x577e2b` sessões, `0x577dc0`/`0x577e2f` roster, `0x578b00` net_game_started, `0x578b08` net_active, `0x579220` net_horizon, `0x579214` net_host_horizon, `0x579080` net_catchup_floor, `0x57921c` net_batch_seq, `0x574c94` net_out_list, `0x574c18` world_ready, `0x574c04` world_loading, `0x574840` setup_block, `0x55a9ac` net_msg_reliable.

## 10. Implementação no clone

Fase 1 (núcleo, sem interface): `crates/rt2-sim/src/net.rs` + `tick/net.rs` (regras da simulação em rede) e o crate `crates/rt2-net` (transporte e lockstep). O formato das mensagens está em `docs/net-protocol.md`. As telas K1–K3/H3 e o chat na tela são a fase 2 (`rt2-game`).

### 10.1 Modo rede da simulação

"Rede" é `World::is_network()` = existe um `NetSession` com **2 ou mais** jogadores no roster (`net_player_count() >= 2`). Sem sessão, ou com um jogador só, o mundo é o de um jogador, intocado (placar de fidelidade `scripts/fidelity.py`: os 134 casos continuam iguais). Cada item abaixo cita a seção desta spec e o que foi conferido no código do EXE (`re/db/decomp/`).

| regra | onde no clone | conferido |
|---|---|---|
| RNG re-semeado todo dia: `rng_seed(ftol((f64)(T + game_seed) + caixa da pessoa 0))` antes do passo de semana/mês; `T` = minuto 23:59 do dia que acaba; a soma `T + game_seed` é o inteiro de 64 bits com a parte alta zero (`fild qword` de `0x4a5cb0`), ou seja, `u32`; pessoa 0 = primeiro id do conjunto de jogadores (`0x46a8a0(0)`) | `SimState::net_day_start` (`tick/net.rs`) | lido em `sim_advance` `0x4a5cb0` e `0x46a8a0` [V] |
| `net_game_start` semeia com `game_seed` | `World::net_begin` | spec 1.7 [S] |
| Trens andam em passos diários: `trains_advance(1440)` = `train_step(trem, 1440, 1)` para cada trem na ordem da lista, depois da troca de data (inclusive depois de `sim_month_end`); o passo linear **não** move trens em rede (`net_player_count() == 1` é a condição do original) | `SimState::trains_advance`, `linear_pass` | lido em `0x4a58f0` (79 bytes) e `0x4a5cb0` [V] |
| O estado em rede **não depende do tamanho do passo** que a máquina roda (por isso o original faz assim) | teste `crates/rt2-sim/tests/net_mode.rs` (40 dias em passos de 320 e de 1000 minutos, mesmo digest) | teste |
| Sem jornal: o aviso de mudança da economia (`0x46a010`) e o de desdobramento de ações (`0x43d800`) só chamam `news_post` com `net_player_count() < 2`; em rede vão para a lista de mensagens (sem sorteio do CRT, sem bandeira/contador de notícia no estado) | `tick.rs` (`economy_state_step`), `tick/year.rs` | lido em `0x46a010` (`FUN_0049c950` < 2) e `0x43d800` (`net_player_count() < 2`) [V] |
| Escolha de evento = botão 1, sem diálogo; `GameIsMultiPlayer` (`0x573f1c`) gravado uma vez ao iniciar | `World::net_begin` grava o estado; o motor de cenários já respeitava o flag; `advance` ignora o modo interativo em rede | spec 3.2 [V] |
| Velocidade/pausa: comando global, executa na chegada; pausa em rede = 1 minuto de jogo por segundo | `rt2-net` `Session::on_batch` (executa `SetSpeed`/`TogglePause` na chegada); o "1 min/s" não é um relógio: com velocidade 0 o anfitrião só concede `T + 1` por lote e há um lote por segundo (heartbeat), então vem dos quadros | teste `pause_in_a_network_game_runs_one_minute_per_second` |
| Humano que sai vira IA (`net_player_left` `0x49d970`): `+0x122 = 0`, `+0x52f = 0`, `+0x548 = 3`, `+0x267 = 0` (versão de par `>= 0x232b`, sempre no clone); empresa e dinheiro ficam; idempotente; com um humano restante, volta ao modo local | `World::net_player_left`, `Command::NetPlayerLeft` (comando `0x01`) | lido em `0x49d970`: a condição `net_player_count() > 1` é avaliada **depois** da remoção [V] |
| Vitória/derrota por jogador (spec 3.3): o original guarda um "estado local" por máquina; o clone guarda **por pessoa** (`NetOutcome`): a primeira vitória marca o jogo como decidido, o alvo recebe o nível, **todo outro humano** recebe derrota "outro jogador alcançou os objetivos" (LNG 2691); derrota (efeito 4) elimina só o alvo (`player_remove`) e o jogo segue; o jogo continua depois da vitória (LNG 2862) e não há recorde. `World::game_over()` devolve o resultado da pessoa local | `scenario/engine.rs` (`game_over_effect`, ramo `network_active`), `SimState::net_outcomes` | spec 3.3 [V], semântica por pessoa é do clone |
| Pessoa/empresa locais por máquina | **visão**, não regra: `World::net_local_person`/`net_local_company` (a pessoa com `+0x126 ==` id local). A simulação continua com a noção determinística de "empresa mostrada" = primeiro humano (`is_ui_company`, `local_player`), então nenhuma regra lê a máquina em que roda. Isso fecha as fontes de dessincronia latentes do original (3.4: bit do tamanho da estação, histórico anual, sorteio do som de entrega) | decisão do clone |
| Humanos recebem pessoas (`0x479710`, 5.3): passada 0 por identidade `+0x54d`, passada 1 pelos restantes em ordem, passada 2 humanas sem jogador viram IA; escreve `+0x126` e `+0x54d` | `World::net_assign_humans` (chamado por `net_begin`) | spec 5.3 [S] |

**Checksum** (`World::net_digest`, `net_checkpoint`): o original soma 26 bytes (RNG, empresas, pessoas, trens, indústrias) no dia 1 de cada mês e só o anfitrião compara. O clone troca o registro por um **digest de estado completo** (`Digest`: RNG, relógio, roster, empresas, pessoas, trens, indústrias, estações, "resto" = outros pools + bloco de estado + mapa + regras + resultados + hash dos comandos executados com o minuto de cada um), nomeado por grupo (`group`), e mantém o ponto de verificação no mesmo instante (`net_time_sync`: fim do dia 1, antes da re-semeadura). Fora do digest, por serem por máquina ou dependerem do tamanho do passo: `game_speed` (executa na chegada), o estado do CRT, o histórico anual da interface (`0x570a82`, registra a empresa mostrada), a empresa local salva no cabeçalho, a identidade `+0x54d`, o bit de luz do tile (`tile+6 & 0x20`, só recalculado no redesenho) e a **ordem da lista de animações** (`pool_78`, fumaça: liberada na primeira passada depois de expirar). Foi o teste de independência do passo que achou esta última.

**Instantâneo** (`NetSnapshot`): arquivo `.GAM` (`to_file_bytes`) + o que o arquivo não tem (estado dos dois RNG, relógio, velocidade, `game_seed`, roster, resultados, hash dos comandos). É como começa uma partida (todas as máquinas, o anfitrião inclusive, recarregam o mesmo instantâneo e rodam `net_begin`), como entra um jogador no meio e como volta uma máquina que divergiu. Carregar o instantâneo é o que o original faz ao carregar um save no lobby (5.3); o teste `a_snapshot_of_a_real_game_rebuilds_an_equivalent_world` mostra que o mundo recarregado e o que continuou rodando seguem iguais por 70 dias de jogo.

### 10.2 Lockstep (`rt2-net`)

- **Anfitrião sequenciador**, como 1.5: o cliente manda o comando ao anfitrião; o anfitrião junta em lotes `(seq, stamp, horizonte, comandos)`, manda a todos e roda o próprio lote pelo mesmo caminho. Lote quando há comando na fila **ou** 1 s sem lote (batimento). `H_novo = max(H_anterior, T + minutos(2000 ms na velocidade atual) + 1)`; o novo anfitrião após migração soma `minutos(8000 ms)` e nunca menos que `max(horizonte visto, T) + 1`. O carimbo é o horizonte do lote anterior. Os lotes se aplicam em ordem de `seq` (buffer para os que chegam fora de ordem); `SetSpeed`/`TogglePause` rodam na chegada, o resto no minuto carimbado, com a simulação parando exatamente em cada carimbo (a recursão de `sim_advance`).
- **Ritmo do quadro** (`Session::pace`): a fórmula de `0x405540` (folga em segmentos de 1 s, razões 10,0 e 3,0, `dt = H - T` com velocidade 0); `Config::frame_ms` = 100 ms. A simulação não lê relógio: `Session::step(now_ms)` recebe o tempo do chamador (o jogo, ou o tempo lógico de um teste).
- **Transporte** abstrato (`Transport`: conectar, enviar quadro, receber eventos; pode perder, duplicar e reordenar) com confiabilidade em cima (`link`: id, ack, reenvio a cada `resend_ms`, remoção de duplicatas); implementações em memória (`MemHub`, com latência, jitter, perda, duplicação, corte silencioso e queda) e TCP (std + uma thread leitora por conexão). Malha completa: cada convidado liga a todos os outros (necessário para a migração).
- **Início**: o anfitrião manda o instantâneo `Start`; todos recarregam, rodam `net_begin` e avisam `Loaded`; só quando todos carregaram o anfitrião começa a emitir lotes (a barreira de `0x4b34c0`).
- **Silêncio**: batimento depois de 10 s sem enviar, sonda depois de 23 s sem ouvir, corte depois de 90,001 s (configuráveis); o anfitrião vigia todos, o convidado só o anfitrião; conexão fechada vale na hora.
- **Migração**: o menor id restante vira anfitrião (cada convidado calcula o mesmo), manda `HostChanged`, continua o `seq` e recebe os +8 s de horizonte. A remoção do jogador que caiu vai pelo lote como comando `0x01` quando eram 3 ou mais, direta em cada máquina quando eram 2 (`multiplayer.md` 5.1); com um só jogador o mundo volta a ser local.
- **Dessincronização com reação** (melhoria de 6): todo mês cada máquina manda o digest ao anfitrião; ao achar diferença o anfitrião (a) emite `Event::Desync` com os grupos que diferem, (b) pede o estado do ponto de verificação à máquina divergente e grava o **diff campo a campo** (`rt2_formats::savefields::diff`, o mesmo do `rt2-savediff`) em `desync-<minuto>-p<id>.txt` (`Config::dump_dir`), e (c) por padrão manda o próprio estado à máquina divergente (`Resync`).
- **Entrar no meio e reconectar** (extra do clone, 5.2 do original diz que não existe): o anfitrião escolhe o assento (`World::net_free_seat`: pessoa que um humano controlou e que hoje é IA, primeiro por identidade), manda `Welcome`, o instantâneo `Join` (estado + fila de comandos pendentes + `seq` + horizonte) e passa a mandar os lotes; o comando `NetPlayerJoined` entra no próximo lote e muda o roster em todos no mesmo minuto. A identidade (`Config::identity`, guardada em `+0x54d`) faz o jogador que volta reassumir a mesma empresa. Um mapa sem assento livre recusa ("no free seat").
- **Conexão**: `Hello` leva versão do protocolo e `rules_hash` = protocolo + versão do `rt2-sim` + `RULES_REVISION` + hash do executável de onde vieram os dados + conjunto de correções de bugs. Diferença = `Reject`.

### 10.3 Validação

- Contra o original (estático, `re/db/decomp/`): fórmula da re-semeadura, `trains_advance`, `net_player_left`, condições de jornal, `net_sync_checksum_build`. Placar de fidelidade e testes do `rt2-sim`/`rt2-oracle` sem diferença (modo de um jogador intocado).
- Dinâmica com duas instâncias do original no Wine: **não feita** (seção 8, item de `Network Debug Mode`).
- Testes do clone: `crates/rt2-net/tests/{lockstep,faults,tcp}.rs` (CI, mundo sem pools), `real_game.rs` e `crates/rt2-sim/tests/net_mode.rs` (precisam de `RT2_DIR`, locais).

### 10.4 Em aberto no clone

- Validar o remetente de cada comando contra a pessoa que controla a empresa (6, item 8): **não implementado** (o protocolo confia em qualquer cliente, como o original); precisa de uma correção nomeada em `ruleset`.
- Outros `news_post` (demissão de presidente, fusão, troca de presidente: `tick/review.rs`, `tick/fincmd.rs`) não foram conferidos contra `net_player_count()`; só o jornal da economia e o desdobramento foram.
- `0x46b150`/`0x46b290` (suspender e retomar a pré-visualização de construção) são estado de interface e não existem no clone.
- Condições de regra "só o jogador que vê" (`+0x34f`/`+0x350`) avaliam para o primeiro humano, não para a máquina local.
- Migração: se o anfitrião morre depois de mandar o lote `k` a um convidado e antes de mandá-lo ao sucessor, o sucessor emite outro `k`; o checksum seguinte acusa e o `Resync` corrige. Comandos enviados ao anfitrião que caiu e ainda sem lote se perdem (como no original).
- Os instantâneos viajam sem compressão (o original comprime com compressor próprio); NAT e descoberta de servidores ficam para a fase 2.
