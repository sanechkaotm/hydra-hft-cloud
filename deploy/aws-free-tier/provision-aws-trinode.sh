#!/usr/bin/env bash
# =============================================================================
# HYDRA-HFT v4.0 — AUTOMATED AWS FREE TIER TRI-NODE PROVISIONER
# Regions:
#   - Node A (Tokyo)     : ap-northeast-1 (Co-located with Binance AWS Tokyo)
#   - Node B (London)    : eu-west-2      (Kraken FX & Crypto Gateway)
#   - Node C (Frankfurt) : eu-central-1   (Base & Arbitrum L2 Sequencer Hub)
# =============================================================================
set -euo pipefail

INSTANCE_TYPE="${HYDRA_AWS_INSTANCE_TYPE:-t3.micro}"
KEY_NAME="${HYDRA_AWS_KEY_NAME:-hydra-hft-key}"
CAPITAL_EUR="${HYDRA_INITIAL_CAPITAL_EUR:-150.00}"
MESH_PORT=19440

echo "============================================================================="
echo " HYDRA-HFT v4.0 — PROVISIONING AWS FREE TIER TRI-NODE MESH (€${CAPITAL_EUR})"
echo "============================================================================="

REGIONS=("ap-northeast-1" "eu-west-2" "eu-central-1")
ROLES=("tokyo" "london" "frankfurt")
declare -A PUBLIC_IPS
declare -A SG_IDS

for idx in 0 1 2; do
    REGION="${REGIONS[$idx]}"
    ROLE="${ROLES[$idx]}"
    echo "[Step 1.${idx}] Resolving latest Ubuntu 24.04 LTS HVM AMI in ${REGION} (${ROLE})..."

    AMI_ID=$(aws ec2 describe-images \
        --region "${REGION}" \
        --owners 099720109477 \
        --filters "Name=name,Values=ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-amd64-server-*" "Name=state,Values=available" \
        --query 'sort_by(Images, &CreationDate)[-1].ImageId' \
        --output text)

    SG_ID=$(aws ec2 create-security-group \
        --region "${REGION}" \
        --group-name "hydra-hft-${ROLE}-sg" \
        --description "Hydra-HFT v4.0 Low-Latency UDP Mesh & SSH (${ROLE})" \
        --query 'GroupId' \
        --output text 2>/dev/null || \
        aws ec2 describe-security-groups \
            --region "${REGION}" \
            --group-names "hydra-hft-${ROLE}-sg" \
            --query 'SecurityGroups[0].GroupId' \
            --output text)

    SG_IDS["${REGION}"]="${SG_ID}"

    # Allow SSH administration
    aws ec2 authorize-security-group-ingress \
        --region "${REGION}" \
        --group-id "${SG_ID}" \
        --protocol tcp \
        --port 22 \
        --cidr 0.0.0.0/0 2>/dev/null || true

    echo "[Step 1.${idx}] Launching Free-Tier ${INSTANCE_TYPE} instance in ${REGION} (${ROLE})..."
    INSTANCE_ID=$(aws ec2 run-instances \
        --region "${REGION}" \
        --image-id "${AMI_ID}" \
        --instance-type "${INSTANCE_TYPE}" \
        --security-group-ids "${SG_ID}" \
        --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=hydra-hft-${ROLE}},{Key=Role,Value=${ROLE}}]" \
        --query 'Instances[0].InstanceId' \
        --output text)

    aws ec2 wait instance-running --region "${REGION}" --instance-ids "${INSTANCE_ID}"

    PUB_IP=$(aws ec2 describe-instances \
        --region "${REGION}" \
        --instance-ids "${INSTANCE_ID}" \
        --query 'Reservations[0].Instances[0].PublicIpAddress' \
        --output text)

    PUBLIC_IPS["${ROLE}"]="${PUB_IP}"
    echo " -> Node [${ROLE}] LIVE at Public IP: ${PUB_IP} (${REGION})"
done

echo "-----------------------------------------------------------------------------"
echo "[Step 2] Locking down 64-byte BLAKE3 UDP Mesh (Port ${MESH_PORT}/udp) to Peer IPs..."
for idx in 0 1 2; do
    REGION="${REGIONS[$idx]}"
    SG_ID="${SG_IDS[$REGION]}"
    for peer_role in "tokyo" "london" "frankfurt"; do
        PEER_IP="${PUBLIC_IPS[$peer_role]}"
        aws ec2 authorize-security-group-ingress \
            --region "${REGION}" \
            --group-id "${SG_ID}" \
            --protocol udp \
            --port "${MESH_PORT}" \
            --cidr "${PEER_IP}/32" 2>/dev/null || true
    done
done

echo "============================================================================="
echo " TRI-NODE AWS FREE TIER MESH PROVISIONED SUCCESSFULLY:"
echo "   - Node A (Tokyo     | ap-northeast-1) : ${PUBLIC_IPS[tokyo]}:${MESH_PORT}"
echo "   - Node B (London    | eu-west-2)      : ${PUBLIC_IPS[london]}:${MESH_PORT}"
echo "   - Node C (Frankfurt | eu-central-1)   : ${PUBLIC_IPS[frankfurt]}:${MESH_PORT}"
echo "============================================================================="
echo " Next: Run on each host:"
echo "   sudo bash deploy/tune-host.sh 1"
echo "   ./target/release/hydra-node --paper-48h --capital ${CAPITAL_EUR} --polls 0"
echo "============================================================================="
