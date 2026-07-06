--[[
KiNET Wireshark Lua Dissector

This is very much AI generated, don't trust it. Built based on:
    - https://github.com/OpenLightingProject/ola/blob/master/plugins/kinet/kinet.cpp
    - https://github.com/Firescar96/kinet/blob/master/kinet/kinet.py
and some of my own reverse engineering

--]] local kinet = Proto("kinet", "KiNET Lighting Protocol")

local MAGIC = 0x0401DC4A
local VERSION = 0x0001
local UDP_PORT = 6038

local COMMON_HEADER_LEN = 8
local DMXOUT_HEADER_LEN = 21

local packet_types = {
    [0x0001] = "Discover Supplies",
    [0x0002] = "Discover Supplies Reply",
    [0x0003] = "Set IP",
    [0x0005] = "Set Universe",
    [0x0006] = "Set Name",
    [0x0008] = "Heartbeat",
    [0x0101] = "DMX Out",
    [0x0201] = "Discover Fixtures Serial Request",
    [0x0202] = "Discover Fixtures Serial Reply",
    [0x0203] = "Discover Fixtures Channel Request",
    [0x0204] = "Discover Fixtures Channel Reply"
}

-- Common fields
local pf_magic = ProtoField.uint32("kinet.magic", "Magic", base.HEX)
local pf_version = ProtoField.uint16("kinet.version", "Version", base.DEC)
local pf_type = ProtoField.uint16("kinet.type", "Packet Type", base.HEX, packet_types)

-- DMX Out fields
local pf_sequence = ProtoField.uint32("kinet.sequence", "Sequence", base.DEC)
local pf_port = ProtoField.uint8("kinet.port", "DMX Port", base.DEC)
local pf_padding = ProtoField.uint8("kinet.padding", "Padding", base.HEX)
local pf_flags = ProtoField.uint16("kinet.flags", "Flags", base.HEX)
local pf_timer = ProtoField.uint32("kinet.timer", "Timer", base.HEX)
local pf_universe = ProtoField.uint8("kinet.universe", "Universe", base.DEC)

-- Discover Supplies fields
local pf_controller_ip = ProtoField.ipv4("kinet.controller_ip", "Magical controller source IP")

-- Discover Supplies Reply fields
local pf_src_ip = ProtoField.ipv4("kinet.src_ip", "Source IP")
local pf_mac = ProtoField.ether("kinet.mac", "MAC Address")
local pf_data = ProtoField.bytes("kinet.data", "Data")
local pf_serial = ProtoField.uint32("kinet.serial", "Serial", base.HEX)
local pf_reserved32 = ProtoField.uint32("kinet.reserved32", "Reserved", base.HEX)
local pf_node_name = ProtoField.string("kinet.node_name", "Node Name")
local pf_node_label = ProtoField.string("kinet.node_label", "Node Label")
local pf_reserved16 = ProtoField.uint16("kinet.reserved16", "Reserved", base.HEX)

-- Discover Fixtures Serial Request/Reply
local pf_target_ip = ProtoField.ipv4("kinet.target_ip", "Target IP")
local pf_fixture_serial = ProtoField.uint32("kinet.fixture_serial", "Fixture Serial", base.HEX)

-- Generic fields
local pf_payload = ProtoField.bytes("kinet.payload", "Payload")

kinet.fields = {pf_magic, pf_version, pf_type, pf_sequence, pf_port, pf_padding, pf_flags, pf_timer, pf_universe,
                pf_controller_ip, pf_src_ip, pf_mac, pf_data, pf_serial, pf_reserved32, pf_node_name, pf_node_label,
                pf_reserved16, pf_payload, pf_target_ip, pf_fixture_serial}

-- Dissector
function kinet.dissector(buffer, pinfo, tree)

    if buffer:len() < COMMON_HEADER_LEN then
        return 0
    end

    if buffer(0, 4):uint() ~= MAGIC then
        return 0
    end

    local version = buffer(4, 2):le_uint()
    local pkt_type = buffer(6, 2):le_uint()

    pinfo.cols.protocol = "KiNET"

    local type_name = packet_types[pkt_type] or string.format("Unknown KiNET packet: 0x%04X", pkt_type)

    local subtree = tree:add(kinet, buffer(), "KiNET Protocol")

    subtree:add(pf_magic, buffer(0, 4))
    subtree:add_le(pf_version, buffer(4, 2))
    subtree:add_le(pf_type, buffer(6, 2))

    if version ~= VERSION then
        subtree:add_expert_info(PI_PROTOCOL, PI_WARN,
            string.format("Unexpected protocol version %u (expected %u)", version, VERSION))
    end

    -- DISCOVER SUPPLIES (0x0001)
    if pkt_type == 0x0001 then
        local off = COMMON_HEADER_LEN

        -- All KiNET packets include the sequence field (usually zeros here)
        if buffer:len() >= off + 4 then
            subtree:add_le(pf_sequence, buffer(off, 4))
            off = off + 4
        end

        -- Controller IP address
        if buffer:len() >= off + 4 then
            subtree:add(pf_controller_ip, buffer(off, 4))
            local ip_string = tostring(buffer(off, 4):ipv4())
            pinfo.cols.info = string.format("%s (Magical IP: %s)", type_name, ip_string)
            off = off + 4
        else
            pinfo.cols.info = type_name
        end

        -- Any trailing payload
        if buffer:len() > off then
            subtree:add(pf_payload, buffer(off))
        end

        -- DMX OUT (0x0101)

    elseif pkt_type == 0x0101 then

        if buffer:len() < DMXOUT_HEADER_LEN then
            return buffer:len()
        end

        local sequence = buffer(8, 4):le_uint()
        local port = buffer(12, 1):uint()
        local universe = buffer(20, 1):uint()

        subtree:add_le(pf_sequence, buffer(8, 4))
        subtree:add(pf_port, buffer(12, 1))
        subtree:add(pf_padding, buffer(13, 1))
        subtree:add_le(pf_flags, buffer(14, 2))
        subtree:add_le(pf_timer, buffer(16, 4))
        subtree:add(pf_universe, buffer(20, 1))

        pinfo.cols.info = string.format("%s Seq=%u Port=%u Universe=%u", type_name, sequence, port, universe)

        if buffer:len() > DMXOUT_HEADER_LEN then
            subtree:add(pf_payload, buffer(DMXOUT_HEADER_LEN))
        end

        -- DISCOVER SUPPLIES REPLY (0x0002)
    elseif pkt_type == 0x0002 then

        local off = COMMON_HEADER_LEN

        -- All KiNET packets include the sequence field. Meaningful only for DMX Out.
        subtree:add_le(pf_sequence, buffer(off, 4))
        off = off + 4

        if buffer:len() >= off + 113 then

            subtree:add(pf_src_ip, buffer(off, 4))
            off = off + 4

            subtree:add(pf_mac, buffer(off, 6))
            off = off + 6

            subtree:add_le(pf_data, buffer(off, 2))
            off = off + 2

            local serial = buffer(off, 4):le_uint()
            subtree:add_le(pf_serial, buffer(off, 4))
            off = off + 4

            subtree:add_le(pf_reserved32, buffer(off, 4))
            off = off + 4

            local node_name = buffer(off, 60):stringz()
            subtree:add(pf_node_name, buffer(off, 60)):set_text("Node Name: " .. node_name)
            off = off + 60

            local node_label = buffer(off, 31):stringz()
            local t = subtree:add(pf_node_label, buffer(off, 31))
            t:set_text("Node Label: " .. node_label)
            off = off + 31

            subtree:add_le(pf_reserved16, buffer(off, 2))

            pinfo.cols.info = string.format("%s Serial=%08X Name=\"%s\"", type_name, serial, node_name)

        else
            pinfo.cols.info = type_name
            subtree:add(pf_payload, buffer(COMMON_HEADER_LEN))
        end

        -- Heartbeat
    elseif pkt_type == 0x0008 then

        local off = COMMON_HEADER_LEN

        -- All KiNET packets include the sequence field. Meaningful only for DMX Out.
        subtree:add_le(pf_sequence, buffer(off, 4))
        off = off + 4

        subtree:add(pf_src_ip, buffer(off, 4))
        off = off + 4

        subtree:add(pf_mac, buffer(off, 6))
        off = off + 6

        subtree:add_le(pf_data, buffer(off, 2))
        off = off + 2

        local serial = buffer(off, 4):le_uint()
        subtree:add_le(pf_serial, buffer(off, 4))
        off = off + 4

        subtree:add_le(pf_reserved32, buffer(off, 4))
        off = off + 4

        pinfo.cols.info = string.format("%s Serial=%08X", type_name, serial)

        -- Discover Fixtures Serial Request
    elseif pkt_type == 0x0201 then

        local off = COMMON_HEADER_LEN

        subtree:add(pf_target_ip, buffer(off, 4))
        off = off + 4

        -- Any trailing payload
        if buffer:len() > off then
            subtree:add(pf_payload, buffer(off))
        end

        pinfo.cols.info = type_name

        -- Discover Fixtures Serial Reply
    elseif pkt_type == 0x0202 then

        local off = COMMON_HEADER_LEN

        subtree:add(pf_src_ip, buffer(off, 4))
        off = off + 4

        local fixture_serial = buffer(off, 4):le_uint()
        subtree:add_le(pf_fixture_serial, buffer(off, 4))
        off = off + 4

        -- Any trailing payload
        if buffer:len() > off then
            subtree:add(pf_payload, buffer(off))
        end

        pinfo.cols.info = string.format("%s FSerial=%08X", type_name, fixture_serial)

        -- Discover Fixtures Channel Request
    elseif pkt_type == 0x0203 then

        local off = COMMON_HEADER_LEN

        subtree:add_le(pf_sequence, buffer(off, 4))
        off = off + 4

        local fixture_serial = buffer(off, 4):le_uint()
        subtree:add_le(pf_fixture_serial, buffer(off, 4))
        off = off + 4

        -- Any trailing payload
        if buffer:len() > off then
            subtree:add(pf_payload, buffer(off))
        end

        pinfo.cols.info = string.format("%s FSerial=%08X", type_name, fixture_serial)

        -- Discover Fixtures Channel Reply
    elseif pkt_type == 0x0204 then

        local off = COMMON_HEADER_LEN

        subtree:add_le(pf_sequence, buffer(off, 4))
        off = off + 4

        local fixture_serial = buffer(off, 4):le_uint()
        subtree:add_le(pf_fixture_serial, buffer(off, 4))
        off = off + 4

        -- Any trailing payload
        if buffer:len() > off then
            subtree:add(pf_payload, buffer(off))
        end

        pinfo.cols.info = string.format("%s FSerial=%08X", type_name, fixture_serial)

        -- EVERYTHING ELSE
    else
        pinfo.cols.info = type_name

        if buffer:len() > COMMON_HEADER_LEN then
            subtree:add(pf_payload, buffer(COMMON_HEADER_LEN))
        end
    end

    return buffer:len()
end

DissectorTable.get("udp.port"):add(UDP_PORT, kinet)
