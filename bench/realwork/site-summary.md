| site | model | effort | runs | best run | judge (anchor) | tests | $ | in / cache read / write / out | cached | min | turns | run tok/s | call tok/s | avg judge | avg $ | avg min |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Experiential Labs | deepseek-v4.1-flash | max | 2 | tazuna-ownel-v39 r0 | 9.5 (—) | 3/3 | 0.204 est | 218K / 6.14M / 0K / 85K | 97% | 18.6 | 91 | 75.7 | 83.8 | 9.25 | 0.145 | 14.25 |
| CheaperInference | deepseek-v4.1-flash | max | 2 | tazuna-own-v38 r0 | 9 (9) | 3/3 | 0.024 est | 99K / 4.34M / 0K / 35K | 98% | 26.2 | 77 | 22.3 | 18.7 | 9 | 0.038 | 20.8 |
| UnoRouter | deepseek-v4.1-flash-1017 | max | 1 | tazuna-ownuno-v111 r0 | 9 (8) | 3/3 | 0.026 | 1.20M / 2.41M / 0K / 35K | 67% | 10.0 | 59 | 58.2 | 48.9 | 9 | 0.026 | 10.0 |
| BeatAPI | deepseek-v4.1-flash-hc | max | 5 | tazuna-ownbeat-v52 r0 | 9 (9) | 3/3 | 0.027 est | 84K / 3.09M / 0K / 28K | 97% | 10.7 | 58 | 44.4 | 56.3 | 7.6 | 0.183 | 75.08 |
| SubRouter | deepseek-v4.1-flash-1017 | max | 5 | tazuna-ownsub-v114 r0 | 9 (8) | 3/3 | 0.046 | 1.25M / 8.80M / 0K / 178K | 88% | 37.3 | 113 | 79.6 | 46.5 | 8 | 0.064 | 30.98 |
| UnoRouter | deepseek-v4.1-flash | max | 8 | tazuna-ownuno-v76 r0 | 9 (8) | 3/3 | 0.055 est | 3.09M / 7.62M / 0K / 117K | 71% | 33.8 | 134 | 57.4 | 58.6 | 7.167 | 0.039 | 23.562 |
| SubRouter | gpt-6-luna | xhigh | 4 | tazuna-ownsubm-v129 r0 | 8 (9) | 3/3 | 0.005 | 91K / 3.62M / 0K / 29K | 98% | 7.6 | 67 | 64.2 | 55.7 | 6.25 | 0.088 | 32.225 |
| UnoRouter | gpt-6.1-sol | max | 1 | tazuna-ownuno-v58 r0 | 8 (9) | 3/3 | 0.015 est | 214K / 4.11M / 0K / 29K | 95% | 22.4 | 70 | 21.5 | 24.8 | 8 | 0.015 | 22.4 |
| Mortera | deepseek-v4.1-flash | max | 2 | tazuna-ownmrt-v98 r0 | 8 (8) | 3/3 | 0.018 | 77K / 3.40M / 0K / 42K | 98% | 8.9 | 74 | 78.6 | 92.2 | 8 | 0.059 | 28.25 |
| SubRouter | gpt-6.1-sol | xhigh | 2 | tazuna-ownsubm-v137 r0 | 8 (9) | 3/3 | 0.019 | 135K / 1.54M / 0K / 21K | 92% | 13.7 | 33 | 25.3 | 28.9 | 7.5 | 0.013 | 14.75 |
| BeatAPI | claude-haiku-5-5 | max | 3 | tazuna-ownbeat-v60 r0 | 8 (9) | 3/3 | 0.031 est | 106K / 1.87M / 0K / 64K | 95% | 14.5 | 45 | 73.1 | 89.3 | 6 | 0.041 | 17.633 |
| SubRouter | gpt-6-astra | xhigh | 1 | tazuna-ownsubm-v140 r0 | 8 (9) | 3/3 | 0.120 | 86K / 1.76M / 0K / 29K | 95% | 14.4 | 38 | 33.4 | 23.6 | 8 | 0.120 | 14.4 |
| Anthropic plan | claude-haiku-5-5 | high | 4 | tazuna-repo-v77 r1 | 8 (9) | 3/3 | 0.326 est | 0K / 4.59M / 140K / 75K | 97% | 7.6 | 54 | 163.7 | 236.7 | 6.75 | 1.784 | 22.4 |
| BeatAPI | gpt-6.1-sol | max | 1 | tazuna-ownbeat-v59 r0 | 8 (9) | 3/3 | 0.348 est | 111K / 3.02M / 0K / 35K | 96% | 19.4 | 42 | 29.7 | 24.7 | 8 | 0.348 | 19.4 |
| BeatAPI | gpt-6-luna | max | 7 | tazuna-ownbeat-v57 r0 | 8 (—) | 3/3 | 0.522 est | 4.73M / 2.08M / 0K / 57K | 31% | 19.1 | 96 | 49.9 | 33.0 | 6.571 | 0.360 | 30.014 |
| OpenAI plan (Codex CLI) | gpt-6-luna | max | 1 | codex r0 | 7 (9) | 3/3 | plan | 179K / 4.55M / 0K / 57K | 96% | 7.5 | 106 | 125.6 | — | 7 | 0.000 | 7.5 |
| Shannon | deepseek-v4.1-flash | xhigh | 3 | tazuna-ownshn-v135 r0 | 7 (9) | 3/3 | 0.004 est | 5.23M / 8.14M / 0K / 150K | 61% | 39.1 | 142 | 63.8 | 32.4 | 6.667 | 0.051 | 43.267 |
| SubRouter (Codex CLI) | gpt-6-luna | max | 1 | codexsub-v138 r0 | 7 (9) | 3/3 | 0.004 | 224K / 5.60M / 0K / 45K | 96% | 11.3 | 162 | 66.4 | — | 7 | 0.004 | 11.3 |
| Mortera | claude-haiku-5-5 | max | 1 | tazuna-ownmrt-v105 r0 | 7 (9) | 3/3 | 0.011 est | 0K / 0K / 0K / 144K | — | 19.0 | 33 | 126.5 | 151.5 | 7 | 0.011 | 19.0 |
| UnoRouter | mimo-v2.6-flash | max | 1 | tazuna-ownuno-v70 r0 | 7 (9) | 3/3 | 0.019 est | 317K / 20.21M / 0K / 92K | 98% | 105.1 | 211 | 14.7 | 9.6 | 7 | 0.019 | 105.1 |
| CheaperInference | gpt-6-luna | max | 1 | tazuna-own-v38 r0 | 7 (—) | 3/3 | 0.133 est | 199K / 14.36M / 0K / 81K | 99% | 24.2 | 119 | 56.2 | 62.2 | 7 | 0.133 | 24.2 |
| SubRouter | gpt-6-sol | xhigh | 1 | tazuna-ownsubm-v165 r0 | 7 (9) | 3/3 | 0.159 est | 252K / 11.02M / 0K / 48K | 98% | 55.2 | 113 | 14.5 | 9.8 | 7 | 0.159 | 55.2 |
| SubRouter (Grok pool key) | grok-4.7 | high | 1 | tazuna-ownsubgp-v161 r0 | 6 (9) | 3/3 | 0.020 | 330K / 2.55M / 0K / 64K | 88% | 20.7 | 40 | 51.3 | 30.5 | 6 | 0.020 | 20.7 |
| SubRouter | gpt-6-sol-plus | xhigh | 1 | tazuna-ownsubm-v170 r0 | 6 (9) | 3/3 | 0.178 | 1.16M / 13.47M / 0K / 66K | 92% | 83.1 | 125 | 13.1 | 10.8 | 6 | 0.178 | 83.1 |
| UnoRouter | gpt-6-luna | max | 6 | tazuna-ownuno-v75 r0 | 6 (8) | 3/3 | 0.199 est | 476K / 27.41M / 0K / 255K | 98% | 159.2 | 210 | 26.7 | 30.3 | 3.4 | 0.044 | 50.85 |
| Anthropic plan | claude-opus-5-5 | medium | 2 | tazuna-repo-v34 r1 | 6 (9) | 1/2 | 1.162 est | 0K / 1.27M / 71K / 26K | 95% | 7.3 | 25 | 60.3 | 118.4 | 6 | 1.310 | 14.4 |
| Mortera | gpt-6-luna | max | 3 | tazuna-ownmrt-v107 r0 | 5 (9) | 3/3 | 0.012 | 200K / 6.51M / 0K / 29K | 97% | 17.9 | 99 | 27.5 | 30.6 | 4.333 | 0.025 | 37.9 |
| SubRouter | gpt-5.6-sol | xhigh | 1 | tazuna-ownsubm-v163 r0 | 5 (9) | 2/3 | 0.040 | 178K / 3.93M / 0K / 35K | 96% | 17.7 | 81 | 33.2 | 20.9 | 5 | 0.040 | 17.7 |
| apinex | claude-opus-5.5 | medium | 1 | tazuna-ownapx-v52 r0 | 5 (9) | 2/3 | 0.305 est | 523K / 0K / 0K / 17K | 0% | 6.5 | 25 | 44.7 | 34.8 | 5 | 0.305 | 6.5 |
| SubRouter | gpt-6.1-sol-new | xhigh | 1 | tazuna-ownsubm-v168 r0 | 4 (9) | 2/3 | 0.003 | 4K / 400K / 0K / 3K | 99% | 12.0 | 31 | 3.9 | 37.4 | 4 | 0.003 | 12.0 |
| SubRouter | gpt-6.1-sol-plus | xhigh | 1 | tazuna-ownsubm-v169 r0 | 4 (9) | 3/3 | 0.096 | 291K / 9.29M / 0K / 40K | 97% | 64.1 | 103 | 10.5 | 10.8 | 4 | 0.096 | 64.1 |
| Shannon | gpt-6.1-sol | xhigh | 1 | tazuna-ownshn-v136 r0 | 4 (9) | 2/3 | 0.170 | 2.20M / 8.80M / 0K / 120K | 80% | 99.4 | 103 | 20.1 | 12.3 | 4 | 0.170 | 99.4 |
| UnoRouter | claude-haiku-5-5 | max | 4 | tazuna-ownuno-v61 r0 | 4 (9) | 2/3 | 0.294 est | 1.52M / 4.87M / 0K / 174K | 76% | 33.7 | 91 | 85.9 | 41.9 | 2.5 | 0.088 | 13.575 |
| SubRouter | claude-opus-5-5 | xhigh | 1 | tazuna-ownsubm-v156 r0 | 3 (9) | 2/3 | 0.467 | 2.12M / 8.78M / 76K / 54K | 80% | 19.9 | 129 | 44.8 | 38.2 | 3 | 0.467 | 19.9 |
| SubRouter | claude-haiku-5-5-mh-kiro | xhigh | 1 | tazuna-ownsubhk-v121 r0 | 2 (9) | 2/3 | 0.006 | 2.02M / 5.35M / 0K / 27K | 73% | 11.2 | 127 | 39.8 | 22.4 | 2 | 0.006 | 11.2 |
| SubRouter | claude-opus-4-8 | xhigh | 1 | tazuna-ownsubm-v145 r0 | 1 (9) | 0/3 | 0.072 | 26K / 2.12M / 185K / 29K | 91% | 11.4 | 73 | 43.1 | 27.0 | 1 | 0.072 | 11.4 |
| BeatAPI | deepseek-v4.1-flash | max | 1 | tazuna-ownbeat-v52 r0 | 1 (9) | 1/3 | 0.191 est | 1.09M / 685K / 0K / 43K | 39% | 16.1 | 39 | 44.3 | 36.1 | 1 | 0.191 | 16.1 |
