import cv2
import mediapipe as mp
import socket
import json
import math

mp_hands = mp.solutions.hands
hands = mp_hands.Hands(
    max_num_hands=2,
    min_detection_confidence=0.6,
    min_tracking_confidence=0.8
)
mp_drawing = mp.solutions.drawing_utils

sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
server_address = ('127.0.0.1', 5005)

cap = cv2.VideoCapture(0)

pinch_state = {'Right': False, 'Left': False}
prev_middle_y = {'Right': 0.0, 'Left': 0.0}

def get_gesture(landmarks):
    # 手首
    wrist = landmarks[0]
    
    # 指先とMCP(付け根)のインデックス
    finger_tips = [8, 12, 16, 20]
    finger_mcps = [5, 9, 13, 17]
    
    folded_count = 0
    for tip_idx, mcp_idx in zip(finger_tips, finger_mcps):
        tip = landmarks[tip_idx]
        mcp = landmarks[mcp_idx]
        
        # 手首からの距離の2乗
        dist_tip = (tip.x - wrist.x)**2 + (tip.y - wrist.y)**2 + (tip.z - wrist.z)**2
        dist_mcp = (mcp.x - wrist.x)**2 + (mcp.y - wrist.y)**2 + (mcp.z - wrist.z)**2
        
        # 指先の方が手首に近い＝折れ曲がっている
        if dist_tip < dist_mcp:
            folded_count += 1
            
    if folded_count >= 3:
        return "Fist"
    elif folded_count == 0:
        return "Open"
    else:
        return "Neutral"

while cap.isOpened():
    success, image = cap.read()
    if not success:
        continue

    image = cv2.flip(image, 1)
    image_rgb = cv2.cvtColor(image, cv2.COLOR_BGR2RGB)
    results = hands.process(image_rgb)

    hand_data_list = []
    snap_detected = False

    if results.multi_hand_landmarks:
        for i, hand_landmarks in enumerate(results.multi_hand_landmarks):
            
            handedness = results.multi_handedness[i].classification[0].label
            
            landmark_list = []
            for id, lm in enumerate(hand_landmarks.landmark):
                landmark_list.append({
                    'id': id,
                    'x': lm.x,
                    'y': lm.y,
                    'z': lm.z
                })
            
            gesture = get_gesture(hand_landmarks.landmark)

            hand_data_list.append({
                'label': handedness,
                'landmarks': landmark_list,
                'gesture': gesture
            })
            
            th = hand_landmarks.landmark[4]
            mi = hand_landmarks.landmark[12]
            
            dist_sq = (th.x - mi.x)**2 + (th.y - mi.y)**2 + (th.z - mi.z)**2
            threshold_sq = 0.004
            
            is_pinching = dist_sq < threshold_sq

            current_y = mi.y
            velocity = abs(current_y - prev_middle_y.get(handedness, current_y))
            velocity_threshold = 0.04 

            if pinch_state.get(handedness, False):
                if not is_pinching:
                    if velocity > velocity_threshold:
                        snap_detected = True
                        print(f"Snap Detected! Hand: {handedness}")
            
            pinch_state[handedness] = is_pinching
            prev_middle_y[handedness] = current_y

            mp_drawing.draw_landmarks(
                image,
                hand_landmarks,
                mp_hands.HAND_CONNECTIONS
            )

    if hand_data_list:
        data = json.dumps({
            'hands': hand_data_list,
            'snap': snap_detected 
        })
        sock.sendto(data.encode('utf-8'), server_address)

    cv2.imshow('MasterHand Vision', image)
    if cv2.waitKey(5) & 0xFF == 27:
        break

cap.release()
cv2.destroyAllWindows()
