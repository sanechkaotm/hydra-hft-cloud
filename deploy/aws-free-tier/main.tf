# =============================================================================
# HYDRA-HFT v4.0 — MULTI-REGION AWS FREE TIER TERRAFORM / OPENTOFU CONFIG
# Topology:
#   - Node A (Tokyo)     : ap-northeast-1 (Co-located with Binance AWS Tokyo)
#   - Node B (London)    : eu-west-2      (Kraken FX & Crypto Gateway)
#   - Node C (Frankfurt) : eu-central-1   (Base & Arbitrum L2 Sequencer Hub)
# =============================================================================

terraform {
  required_version = ">= 1.5.0"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

variable "instance_type" {
  description = "AWS Free-Tier eligible instance type (t3.micro or t4g.micro)"
  type        = string
  default     = "t3.micro"
}

variable "initial_capital_eur" {
  description = "Starting capital for the 48-hour validation run in EUR"
  type        = string
  default     = "150.00"
}

provider "aws" {
  alias  = "tokyo"
  region = "ap-northeast-1"
}

provider "aws" {
  alias  = "london"
  region = "eu-west-2"
}

provider "aws" {
  alias  = "frankfurt"
  region = "eu-central-1"
}

locals {
  cloud_init_userdata = <<-EOF
    #!/bin/bash
    set -euxo pipefail
    apt-get update && apt-get install -y curl git build-essential docker.io docker-compose-v2
    sysctl -w net.core.busy_poll=50
    sysctl -w net.core.busy_read=50
    sysctl -w net.core.rmem_max=67108864
    sysctl -w net.core.wmem_max=67108864
  EOF
}

# --- Node A: Tokyo (ap-northeast-1) ---
data "aws_ami" "ubuntu_tokyo" {
  provider    = aws.tokyo
  most_recent = true
  owners      = ["099720109477"]
  filter {
    name   = "name"
    values = ["ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-amd64-server-*"]
  }
}

resource "aws_security_group" "hydra_tokyo_sg" {
  provider    = aws.tokyo
  name        = "hydra-hft-tokyo-sg"
  description = "Hydra-HFT Tokyo Node A UDP 19440 Mesh & SSH"

  ingress {
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }
  ingress {
    from_port   = 19440
    to_port     = 19440
    protocol    = "udp"
    cidr_blocks = ["0.0.0.0/0"]
  }
  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

resource "aws_instance" "hydra_tokyo" {
  provider               = aws.tokyo
  ami                    = data.aws_ami.ubuntu_tokyo.id
  instance_type          = var.instance_type
  vpc_security_group_ids = [aws_security_group.hydra_tokyo_sg.id]
  user_data              = local.cloud_init_userdata
  tags = {
    Name = "hydra-hft-node-a-tokyo"
    Role = "tokyo"
  }
}

# --- Node B: London (eu-west-2) ---
data "aws_ami" "ubuntu_london" {
  provider    = aws.london
  most_recent = true
  owners      = ["099720109477"]
  filter {
    name   = "name"
    values = ["ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-amd64-server-*"]
  }
}

resource "aws_security_group" "hydra_london_sg" {
  provider    = aws.london
  name        = "hydra-hft-london-sg"
  description = "Hydra-HFT London Node B UDP 19440 Mesh & SSH"

  ingress {
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }
  ingress {
    from_port   = 19440
    to_port     = 19440
    protocol    = "udp"
    cidr_blocks = ["0.0.0.0/0"]
  }
  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

resource "aws_instance" "hydra_london" {
  provider               = aws.london
  ami                    = data.aws_ami.ubuntu_london.id
  instance_type          = var.instance_type
  vpc_security_group_ids = [aws_security_group.hydra_london_sg.id]
  user_data              = local.cloud_init_userdata
  tags = {
    Name = "hydra-hft-node-b-london"
    Role = "london"
  }
}

# --- Node C: Frankfurt (eu-central-1) ---
data "aws_ami" "ubuntu_frankfurt" {
  provider    = aws.frankfurt
  most_recent = true
  owners      = ["099720109477"]
  filter {
    name   = "name"
    values = ["ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-amd64-server-*"]
  }
}

resource "aws_security_group" "hydra_frankfurt_sg" {
  provider    = aws.frankfurt
  name        = "hydra-hft-frankfurt-sg"
  description = "Hydra-HFT Frankfurt Node C UDP 19440 Mesh & SSH"

  ingress {
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }
  ingress {
    from_port   = 19440
    to_port     = 19440
    protocol    = "udp"
    cidr_blocks = ["0.0.0.0/0"]
  }
  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

resource "aws_instance" "hydra_frankfurt" {
  provider               = aws.frankfurt
  ami                    = data.aws_ami.ubuntu_frankfurt.id
  instance_type          = var.instance_type
  vpc_security_group_ids = [aws_security_group.hydra_frankfurt_sg.id]
  user_data              = local.cloud_init_userdata
  tags = {
    Name = "hydra-hft-node-c-frankfurt"
    Role = "frankfurt"
  }
}

output "mesh_endpoints" {
  value = {
    tokyo_ap_northeast_1   = "${aws_instance.hydra_tokyo.public_ip}:19440"
    london_eu_west_2       = "${aws_instance.hydra_london.public_ip}:19440"
    frankfurt_eu_central_1 = "${aws_instance.hydra_frankfurt.public_ip}:19440"
  }
}
