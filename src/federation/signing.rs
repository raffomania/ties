use activitypub_federation::http_signatures::Keypair;
use anyhow::Result;

// Use a hardcoded keypair for testing since generating one for each test is
// slow.
#[cfg(test)]
const PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDM694SJDJe8ebG
l6MesbIzn/LZ5FO4ZeuM2grerkPxZ2FrWVM16Az5NFH4cySD7uTnzdQEjSkCx/OC
4fGlvcqVmzqtqcwI3wfjMI7ihclNcZG+ZYI9ucrbpWglPJeUr1ORkYswNOuGrw8P
imyrHZfSDxPkChDEbX3QpDi/Au9uxhanLuAxabQ41SaANixUemvKcPGoF9NQzjsq
M8fcaTmy/H1wdeW6Fyf8oz0U5gTL1Smn0xyJMDB/dvUGdbT+S70DQhfND4suCtKQ
KsfpB64kNmmnZ1xw/ehWSEx5Jyj2tP3sQm1dUFvGVDwsPe3bPGwz0TbgzVfovODK
uGYkLDR3AgMBAAECggEADEkDukCzSF/mPveuTtPGZDPtokR/BGoP0hTsW+dEqX9S
JtZnF68+v930IBn/EW3MCV2cnV09HS6RmcIj85TKRWfV/71TPyDn3yX1Gv18UQlC
/JAnW738vGhRMxJL4B0WvH5mQtCZYiyykXLyCtwuUuiWf7BeyvfpeA2wXTs20YQx
NoQr53PxQnD7LBr9OPuxQmef5E0o1aOlw9bMXxOVMEL4zLj/xhvJ0gv/Rn7C8dxB
cuvKcuiVjBpLYa1TT38aGp8oOStl9GCmhCOij9FWLVBwzbQGGAmksLX6KkRw2h6Q
EBSVjRUu6I9t/lorKkFoerPSCMICi2Yj/sfGFib02QKBgQDntdYzzdrpctfTtMdJ
ET20M4dRoVqaj0U83LdKSEMp0x4JAFZHzn6qnxeZ8dS6aXevHxpiPdcC4WdZ6rSW
a28La08E3CCHThZ/vNDvP+ShERKCLSqa+bUQ9klDfn8hGTD6w0d0Mhx8v4wy1IVA
L8UWQY/G2E9D79UCOFgtlGNiswKBgQDiZx/UWrTcu0nG9o5m8h9yMQPt0/qXHaKS
0+7C6dUezjN66Iu+joP4U5agsbFZ+zWVK+xOvZDKy52dlhKAMJREBK3B7igVi/5M
pkC3JAdcIeP6XQ72wo608wLSfgmgnvA5uCb6XP/gegVxaW7z4YI78RAyStGndcQj
UStSkF45LQKBgQCR2He6ZdFr7biB7iEeEbcYDPMY44onDRUUqQzJudBkrBkUq1yj
mAtMlBUD9h7jMu19kgNGYQxMKNqn0z7WC0t7EZFMSs5CvFkXEB8m6L2c0CUpQQq3
P4PD6HHXBPE6QSP+QxpfvgcGUn8Jo5E4BJl2V9AK5i6GYZhe7F48WlFwVwKBgQC4
Wag9Ta6/nXExpUnG4Zhhby/31AfUTLk4PYHJDIYRE24vwnMnsvwalFWue4Ih9r9m
u+ErLIhd2PZ6ftyJrQTNbdHee4IAKYHj/+vqNFgZ2S69ilDI9RsmlEnUA/Tq6QBK
v3xdmKRxsGoGMwe5ZgKZtGyvxuR4KxiOeWWBUTSn8QKBgC7Y8QacsOEFT9KQ6aT7
HQIY5+Pf5CkiBV873MDZl3cd7F9z5WAcCv9EatoCeK1TWjkrHL7NRBz0djMVHSCG
YC8FTA5UGgdd1JwId59Tnj0tI4ynqKy8QS6x5U8SNGx1PhGzcfR8EDoocMP8Y3Wg
NibMISpyRl/tjPoQvtYPcvzb
-----END PRIVATE KEY-----";

#[cfg(test)]
const PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAzOveEiQyXvHmxpejHrGy
M5/y2eRTuGXrjNoK3q5D8Wdha1lTNegM+TRR+HMkg+7k583UBI0pAsfzguHxpb3K
lZs6ranMCN8H4zCO4oXJTXGRvmWCPbnK26VoJTyXlK9TkZGLMDTrhq8PD4psqx2X
0g8T5AoQxG190KQ4vwLvbsYWpy7gMWm0ONUmgDYsVHprynDxqBfTUM47KjPH3Gk5
svx9cHXluhcn/KM9FOYEy9Upp9MciTAwf3b1BnW0/ku9A0IXzQ+LLgrSkCrH6Qeu
JDZpp2dccP3oVkhMeSco9rT97EJtXVBbxlQ8LD3t2zxsM9E24M1X6LzgyrhmJCw0
dwIDAQAB
-----END PUBLIC KEY-----";

#[cfg(test)]
#[expect(
    clippy::unnecessary_wraps,
    reason = "Needs same signature as non-test function"
)]
pub fn generate_keypair() -> Result<Keypair> {
    Ok(Keypair {
        private_key: PRIVATE_KEY.to_owned(),
        public_key: PUBLIC_KEY.to_owned(),
    })
}

#[cfg(not(test))]
pub fn generate_keypair() -> Result<Keypair> {
    use activitypub_federation::http_signatures::generate_actor_keypair;
    Ok(generate_actor_keypair()?)
}
