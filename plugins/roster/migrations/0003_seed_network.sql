-- Day-0 network: the 22 prototype network KOLs by registry slug (data/prototype/arena_data_v2.json).
-- keep = true for the 9 that are absent from the owner's sheet: a sheet read never removes them.
-- The prototype X handle is not verified; staff verifies it on the Network screen.
INSERT INTO roster.member (kol, name, x_handle, fomo_handle, fomo_key, keep) VALUES
    ('iced', 'Iced', 'IcedKnife', NULL, NULL, false),
    ('j7', 'J777Crypto', 'J777Crypto', NULL, NULL, false),
    ('levis', 'Levis', 'LevisNFT', 'LevisNFT', 'levisnft', false),
    ('maurits', 'Maurits', 'mauritsneo', 'mauritsneo', 'mauritsneo', false),
    ('juicy', 'juicy', 'Juicycooks', 'Juicycooks', 'juicycooks', false),
    ('solstice', 'TheSOLstice', 'The__Solstice', 'The__Solstice', 'the__solstice', false),
    ('smokey', 'Smokey', 'smokey0x', 'smokey0x', 'smokey0x', false),
    ('brazilzz', 'BrazilZZ', 'xbrazilzz', 'xbrazilzz', 'xbrazilzz', false),
    ('zagy', 'Zagrebian', 'Zagy998', 'Zagy998', 'zagy998', false),
    ('btcforlife', 'BTCFORLIFE', 'BTCFORLIFE', 'BTCFORLIFE', 'btcforlife', false),
    ('ndf', 'NDF_Sonar_trade', 'NDF_Sonar_trade', 'NDF_Sonar_trade', 'ndf_sonar_trade', false),
    ('trancey', 'trancey', 'tranc3y', 'trancey', 'trancey', false),
    ('kook', 'kook', 'KookCapitalLLC', NULL, NULL, true),
    ('mrpunk', 'Mrpunk.eth', 'mrpunkdoteth', NULL, NULL, true),
    ('ocr', 'ocr', 'ocrxa', NULL, NULL, true),
    ('laseasy', 'laseasy10', '_laseasy', NULL, NULL, true),
    ('tekkerrss', 'Tekkerrss', 'Tekkerrss', NULL, NULL, true),
    ('peter', 'Peter', 'Onepeterrr', NULL, NULL, true),
    ('naka', 'Naka', 'Nakazutra', NULL, NULL, true),
    ('babatooonde', 'babatooonde', 'babatooonde', NULL, NULL, true),
    ('pow', 'pow', 'traderpow', NULL, NULL, false),
    ('shah', 'shah', 'shahh', NULL, NULL, true)
ON CONFLICT (kol) DO NOTHING;

INSERT INTO roster.terms (kol)
SELECT kol FROM roster.member
ON CONFLICT (kol) DO NOTHING;

INSERT INTO roster.membership_event (kol, action, reason)
SELECT kol, 'joined', 'prototype seed' FROM roster.member m
WHERE NOT EXISTS (SELECT 1 FROM roster.membership_event e WHERE e.kol = m.kol);
