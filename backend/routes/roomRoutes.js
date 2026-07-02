const express = require("express");
const router = express.Router();

const {
  createRoom,
  joinRoom,
  getRoom,
} = require("../controllers/roomController");

router.post("/create", createRoom);
router.post("/join", joinRoom);
router.get("/:roomId", getRoom);

module.exports = router;