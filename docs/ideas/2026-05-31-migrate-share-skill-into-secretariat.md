# Migrate the /share skill into Secretariat

**Date:** 2026-05-31
**Status:** someday
**Source:** Things3, triaged 2026-05-31

> Migrate /share skill into Secretariat

Captured detail:

> /share currently drafts to ~/Downloads/ with a footer signature line. Same primitive lives in Secretariat now (compose → stamp → outbox per recipient/channel). Plan migration so /share routes through Secretariat envelope flow instead of loose markdown in Downloads — recipient as Peer or LocalQueue, body inline, signature optional per the configurable closing-line rule. Today's draft (secretariat-vision-for-marcelo.md) is a canonical test case.
